"""Diagnostic CPU sampling of unchanged native probes; instrumented timings are invalid."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import time

import rust_pair_scout as scout

ROOT = scout.ROOT
NAME = "msc_d5_inject_cultivate_p1e-3"
CASES = [("baseline", "strict"), ("candidate", "strict"), ("candidate", "fused"), ("baseline", "fused")]
ENV = {"RUSTFLAGS": "-C target-cpu=native", **{k: "1" for k in scout.ENV_KEYS[1:]}}
PREFLIGHT = ["direct-bits", "highest-gather", "gather-cdf", "frozen-bits", "both-policy-bits", "public-counts"]
EXPORTS = {"perf-script.txt": ["script", "--header"],
           "perf-report.txt": ["report", "--stdio", "--no-children", "--percent-limit", "0"],
           "perf-buildids.txt": ["buildid-list"], "perf-header.txt": ["report", "--header-only"]}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


def choose_perf():
    candidates = [Path("/usr/bin/perf"), *sorted(Path("/usr/lib/linux-tools").glob("*/perf"))]
    for path in candidates:
        if path.is_file():
            result = subprocess.run([str(path), "--version"], capture_output=True, text=True, timeout=10)
            if result.returncode == 0:
                return path.resolve(), result.stdout.strip()
    raise ValueError("no working perf executable; retain setup logs, do not claim a profile")


def command(args, directory, stem):
    """Every subprocess is bounded and reaped; failures keep their original output."""
    stdout, stderr = directory / stem, directory / (stem + ".stderr")
    with stdout.open("wb") as out, stderr.open("wb") as err:
        result = subprocess.run(args, stdout=out, stderr=err, check=False, timeout=180)
    item = dict(command=args, exit_code=result.returncode, outputs={
        p.name: dict(path=p.name, sha256=scout.digest(p)) for p in [stdout, stderr]})
    (directory / (stem + ".command.json")).write_text(json.dumps(item, indent=2) + "\n")
    require(result.returncode == 0, "profile command failed: " + stem)
    return item


def probe_args(root, policy, action, repetitions):
    root = Path(root)
    return [str(scout.probe(root) / "target/release/counts-path-ablation"),
            str(root / "benchmarks/near_clifford/application_counts/fixtures" / (NAME + ".stim")),
            str(scout.probe(root) / (NAME + ".masks.json")), "1024", policy, "native", action, str(repetitions)]


def record_args(perf, cpu, root, policy, data):
    return ["sudo", "-n", "env", *[k + "=" + v for k, v in ENV.items()], str(perf), "record",
            "-e", "cpu-clock:u", "-F", "499", "--call-graph", "dwarf,8192", "-o", str(data),
            "--", "taskset", "-c", str(cpu), *probe_args(root, policy, "bench", 101)]


def verify_command(item, directory, stem, args):
    require(item["command"] == args and item["exit_code"] == 0, "fixed successful command: " + stem)
    require(item == read(directory / (stem + ".command.json")), "original command receipt: " + stem)
    require(set(item["outputs"]) == {stem, stem + ".stderr"}, "exact command outputs: " + stem)
    for name, entry in item["outputs"].items():
        require(entry["path"] == name and scout.digest(directory / name) == entry["sha256"], "bounded original output: " + stem)


def profile(baseline_ref, out):
    require(platform.system() == "Linux" and platform.machine() == "x86_64", "native x86 Linux required")
    require({k: os.environ.get(k) for k in scout.ENV_KEYS} == ENV, "fixed native/thread environment required")
    out = out.resolve()
    require(out.is_relative_to(ROOT / "drafts"), "profile output must be under ignored drafts")
    require(not out.exists(), "preserve previous profile; choose a fresh output directory")
    roots = scout.prepare(baseline_ref)
    perf, version = choose_perf()
    available = sorted(os.sched_getaffinity(0))
    cpu = available[0]
    os.sched_setaffinity(0, {cpu})
    out.mkdir(parents=True)
    retained = scout.retain_probe_inputs(roots, out)
    shutil.copyfile(Path(__file__), out / "original-profile-driver.py")
    shutil.copyfile(ROOT / "benchmarks/near_clifford/verify_x86_rotations.sh", out / "original-native-preflight.sh")
    shutil.copyfile(ROOT / "drafts/x86-scout-features.txt", out / "features.txt")
    require(all(re.search(r"\b" + feature + r"\b", (out / "features.txt").read_text()) for feature in ["avx2", "fma"]), "actual AVX2/FMA feature gate")
    preflight = {}
    for name in PREFLIGHT:
        source = ROOT / ("drafts/x86-scout-" + name + ".log")
        expected = 10 if name == "public-counts" else 1
        require(f"test result: ok. {expected} passed;" in source.read_text(), "native preflight missing: " + name)
        target = out / ("preflight-" + name + ".log")
        shutil.copyfile(source, target)
        preflight[name] = dict(path=target.name, sha256=scout.digest(target))
    before = {role: scout.identity(root) for role, root in roots.items()}
    require(all(i["dirty"] == "" for i in before.values()), "clean sources required")
    header = dict(schema="diagnostic.native-cpu-sampling.v1", started=time.time(), performance_valid=False,
                  scope="Whole-process software CPU samples, including setup/warmup/teardown; instrumented observation timings are invalid; no peer comparison",
                  identities=before, retained=retained, preflight=preflight,
                  profile_driver_sha256=scout.digest(Path(__file__)), available_affinity=available, affinity=[cpu],
                  native_preflight_sha256=scout.digest(out / "original-native-preflight.sh"), features_sha256=scout.digest(out / "features.txt"),
                  host=platform.platform(), lscpu=json.loads(scout.run(["lscpu", "--json"])), rustc=scout.run(["rustc", "-Vv"]),
                  perf=dict(path=str(perf), sha256=scout.digest(perf), version=version), cases=CASES,
                  roots={role: str(root) for role, root in roots.items()}, original_out=str(out), binaries={},
                  event="cpu-clock:u", frequency=499, call_graph="dwarf,8192", observations=101)
    (out / "header.json").write_text(json.dumps(header, indent=2) + "\n")
    binaries = out / "binaries"
    binaries.mkdir()
    for role, root in roots.items():
        shutil.copyfile(scout.probe(root) / "target/release/counts-path-ablation", binaries / (role + ".bin"))
        require(scout.digest(binaries / (role + ".bin")) == before[role]["binary"], "measured binary copy")
        header["binaries"][role] = {
            "notes": command(["readelf", "-n", str(binaries / (role + ".bin"))], binaries, role + ".notes.txt"),
            "symbols": command(["nm", "-SC", "--size-sort", str(binaries / (role + ".bin"))], binaries, role + ".symbols.txt")}
    (out / "header.json").write_text(json.dumps(header, indent=2) + "\n")
    events = []
    for index, (role, policy) in enumerate(CASES):
        directory = out / (str(index) + "-" + role + "-" + policy)
        directory.mkdir()
        root = roots[role]
        validate = command(probe_args(root, policy, "validate", 7), directory, "validation.json")
        checked = read(directory / "validation.json")
        require(checked.get("status") == "ok" and checked.get("exact_records_counts_rng") is True
                and checked.get("native_counts_rng") is True and checked.get("seeds") == 4, "profile input records/RNG validation failed")
        # sudo is limited to this isolated runner's perf child. No sysctl or source
        # change occurs. Explicit environment matches the validated native child.
        args = record_args(perf, cpu, root, policy, directory / "perf.data")
        record = command(args, directory, "instrumented-result.json")
        exports = {}
        for stem, subcommand in EXPORTS.items():
            exports[stem] = command([str(perf), *subcommand, "-i", str(directory / "perf.data")], directory, stem)
        item = dict(index=index, role=role, policy=policy, directory=directory.name, validate=validate, record=record,
                    exports=exports, data_sha256=scout.digest(directory / "perf.data"), performance_valid=False)
        events.append(item)
        with (out / "events.jsonl").open("a") as stream:
            stream.write(json.dumps(item, separators=(",", ":")) + "\n")
    after = {role: scout.identity(root) for role, root in roots.items()}
    require(before == after and os.sched_getaffinity(0) == {cpu}, "profile source/binary/environment/affinity changed")
    scout.verify_retained_inputs(out, before, retained)
    closure = dict(finished=time.time(), identities_after=after, events=4, events_sha256=scout.digest(out / "events.jsonl"),
                   header_sha256=scout.digest(out / "header.json"), performance_valid=False)
    (out / "closure.json").write_text(json.dumps(closure, indent=2) + "\n")
    verify(out, git_sources=True, replay=True)


def verify(out, *, git_sources=True, replay=True):
    """Replay retained input, command, sample and measured-binary bindings."""
    out = Path(out)
    h, c = read(out / "header.json"), read(out / "closure.json")
    require(scout.digest(out / "header.json") == c["header_sha256"], "original header digest")
    require(h["schema"] == "diagnostic.native-cpu-sampling.v1" and h["performance_valid"] is False
            and c["performance_valid"] is False, "instrumented timings must never be performance evidence")
    require(h["cases"] == [list(t) for t in CASES] and (h["event"], h["frequency"], h["call_graph"], h["observations"]) == ("cpu-clock:u", 499, "dwarf,8192", 101), "fixed profile protocol")
    require(h["host"].startswith("Linux-") and "release: 1.93.1\n" in h["rustc"]
            and "host: x86_64-unknown-linux-gnu\n" in h["rustc"], "native pinned host/toolchain")
    require(scout.digest(out / "original-native-preflight.sh") == h["native_preflight_sha256"]
            and scout.digest(out / "features.txt") == h["features_sha256"], "original native feature/preflight bytes")
    require(all(re.search(r"\b" + feature + r"\b", (out / "features.txt").read_text()) for feature in ["avx2", "fma"]), "actual AVX2/FMA features")
    require(len(h["affinity"]) == 1 and h["affinity"][0] == min(h["available_affinity"]), "profile CPU binding")
    require(h["identities"] == c["identities_after"] and set(h["identities"]) == {"baseline", "candidate"}, "source/binary closure")
    require(all(i["dirty"] == "" and i["environment"] == ENV for i in h["identities"].values()), "clean native source environment")
    require(all(re.fullmatch(r"[0-9a-f]{40}", i["head"]) for i in h["identities"].values()), "exact source commits")
    require(set(h["roots"]) == {"baseline", "candidate"} and Path(h["roots"]["candidate"]).is_absolute()
            and Path(h["roots"]["baseline"]) == Path(h["roots"]["candidate"]) / "drafts/rust-pair-scout-baseline", "bounded source roots")
    require(Path(h["original_out"]).is_relative_to(Path(h["roots"]["candidate"]) / "drafts"), "bounded original profile output")
    require(all(type(t) in [int, float] and math.isfinite(t) for t in [h["started"], c["finished"]]) and c["finished"] >= h["started"], "closed profile time")
    scout.verify_retained_inputs(out, h["identities"], h["retained"])
    require(scout.digest(out / "original-profile-driver.py") == h["profile_driver_sha256"], "profile driver bytes")
    if git_sources:
        candidate = h["identities"]["candidate"]["head"]
        require((out / "original-profile-driver.py").read_bytes() == subprocess.check_output(
            ["git", "show", candidate + ":benchmarks/near_clifford/native_cpu_profile.py"], cwd=ROOT), "Git profile driver binding")
        require((out / "original-driver.py").read_bytes() == subprocess.check_output(
            ["git", "show", candidate + ":benchmarks/near_clifford/rust_pair_scout.py"], cwd=ROOT), "Git scout driver binding")
        require((out / "original-native-preflight.sh").read_bytes() == subprocess.check_output(
            ["git", "show", candidate + ":benchmarks/near_clifford/verify_x86_rotations.sh"], cwd=ROOT), "Git native preflight binding")
        for role, info in h["identities"].items():
            paths = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", info["head"], "rstim/src"], cwd=ROOT, text=True).splitlines()
            paths = [p for p in paths if p.endswith(".rs")]
            paths += ["Cargo.toml", "Cargo.lock", "rstim/Cargo.toml"]
            paths += ["benchmarks/near_clifford/application_counts/fixtures/" + n + ".stim" for n in scout.NAMES]
            expected = {p: hashlib.sha256(subprocess.check_output(["git", "show", info["head"] + ":" + p], cwd=ROOT)).hexdigest() for p in paths}
            frozen = "benchmarks/near_clifford/results/apple-m4-strict-coefficient-pairs-rust-ablation-2026-10-08/probe/warm-candidate/"
            expected.update({"drafts/rust-pair-scout-probe/" + n: hashlib.sha256(subprocess.check_output(
                ["git", "show", candidate + ":" + frozen + n], cwd=ROOT)).hexdigest() for n in scout.PROBE_INPUTS})
            require(info["sources"] == expected, "full original Git source inventory: " + role)
    require(set(h["preflight"]) == set(PREFLIGHT), "all native gate logs required")
    for name, entry in h["preflight"].items():
        require(entry["path"] == "preflight-" + name + ".log" and scout.digest(out / entry["path"]) == entry["sha256"], "native log bytes")
        require(f"test result: ok. {10 if name == 'public-counts' else 1} passed;" in (out / entry["path"]).read_text(), "native gate result")
    buildids = {}
    require(set(h["binaries"]) == {"baseline", "candidate"}, "binary command inventory")
    for role in ["baseline", "candidate"]:
        original_binary = str(Path(h["original_out"]) / "binaries" / (role + ".bin"))
        require(set(h["binaries"][role]) == {"notes", "symbols"}, "binary exports required")
        verify_command(h["binaries"][role]["notes"], out / "binaries", role + ".notes.txt", ["readelf", "-n", original_binary])
        verify_command(h["binaries"][role]["symbols"], out / "binaries", role + ".symbols.txt", ["nm", "-SC", "--size-sort", original_binary])
        require(scout.digest(out / "binaries" / (role + ".bin")) == h["identities"][role]["binary"], "actual native binary bytes")
        matches = re.findall(r"Build ID: ([0-9a-f]+)", (out / "binaries" / (role + ".notes.txt")).read_text())
        require(len(matches) == 1, "retained native binary build-id")
        buildids[role] = matches[0]
    raw = (out / "events.jsonl").read_bytes()
    require(hashlib.sha256(raw).hexdigest() == c["events_sha256"] and c["events"] == 4, "event digest/count")
    events = [json.loads(line) for line in raw.splitlines()]
    require(len(events) == 4, "complete profile event inventory")
    for index, (e, (role, policy)) in enumerate(zip(events, CASES)):
        require((e["index"], e["role"], e["policy"], e["directory"]) == (index, role, policy, str(index) + "-" + role + "-" + policy)
                and e["performance_valid"] is False, "profile event context")
        directory = out / e["directory"]
        require(set(e["exports"]) == {"perf-script.txt", "perf-report.txt", "perf-buildids.txt", "perf-header.txt"}, "full perf exports")
        original_data = Path(h["original_out"]) / e["directory"] / "perf.data"
        verify_command(e["validate"], directory, "validation.json", probe_args(h["roots"][role], policy, "validate", 7))
        verify_command(e["record"], directory, "instrumented-result.json", record_args(h["perf"]["path"], h["affinity"][0], h["roots"][role], policy, original_data))
        for stem, subcommand in EXPORTS.items():
            verify_command(e["exports"][stem], directory, stem, [h["perf"]["path"], *subcommand, "-i", str(original_data)])
        checked = read(directory / "validation.json")
        require((checked["shots"], checked["policy"], checked["status"], checked["seeds"]) == (1024, policy, "ok", 4)
                and checked["exact_records_counts_rng"] is True and checked["native_counts_rng"] is True, "exact profile validation")
        result = read(directory / "instrumented-result.json")
        require((result["shots"], result["policy"], result["route"]) == (1024, policy, "native") and len(result["observations"]) == 101, "instrumented native context")
        for o in result["observations"]:
            require(type(o["calls"]) is int and o["calls"] > 0 and type(o["elapsed_ns"]) is int and o["elapsed_ns"] >= 50_000_000
                    and math.isfinite(o["ns_per_call"]) and o["ns_per_call"] == o["elapsed_ns"] / o["calls"], "instrumented observation arithmetic")
        data = directory / "perf.data"
        require(data.read_bytes()[:8] == b"PERFILE2" and scout.digest(data) == e["data_sha256"], "original perf data bytes")
        samples = re.findall(r"(?m)^[^#\n]+\s\d+\.\d+:\s+(?:\d+\s+)?cpu-clock:u:", (directory / "perf-script.txt").read_text())
        require(len(samples) >= 100, "at least 100 actual software samples required")
        ids = (directory / "perf-buildids.txt").read_text().splitlines()
        executable = probe_args(h["roots"][role], policy, "bench", 101)[0]
        require(any(line.split(maxsplit=1) == [buildids[role], executable] for line in ids), "samples bound to exact measured executable build-id/path")
    if replay:
        perf, version = choose_perf()
        require(str(perf) == h["perf"]["path"] and scout.digest(perf) == h["perf"]["sha256"] and version == h["perf"]["version"], "trusted local perf matches recorded tool")
        for e in events:
            directory = out / e["directory"]
            for stem, subcommand in EXPORTS.items():
                reproduced = subprocess.check_output([str(perf), *subcommand, "-i", str(directory / "perf.data")], timeout=180)
                require(reproduced == (directory / stem).read_bytes(), "raw perf data export replay: " + stem)
        for role in ["baseline", "candidate"]:
            for stem, args in [(role + ".notes.txt", ["readelf", "-n"]), (role + ".symbols.txt", ["nm", "-SC", "--size-sort"])]:
                reproduced = subprocess.check_output([*args, str(out / "binaries" / (role + ".bin"))], timeout=180)
                require(reproduced == (out / "binaries" / stem).read_bytes(), "actual binary export replay: " + stem)
    return h, c


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-ref")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--verify", type=Path)
    args = parser.parse_args()
    if args.verify:
        verify(args.verify, git_sources=True, replay=True)
    else:
        require(args.baseline_ref is not None and args.out is not None, "baseline-ref and fresh out required")
        profile(args.baseline_ref, args.out)
