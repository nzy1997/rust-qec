"""Fixture-only retained-metadata controls; no perf execution or native evidence claim."""
import copy
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import native_cpu_profile as profile


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def output_command(directory, stem, args, text):
    (directory / stem).write_text(text)
    (directory / (stem + ".stderr")).write_text("")
    item = dict(command=args, exit_code=0, timed_out=False, outputs={
        name: dict(path=name, sha256=profile.scout.digest(directory / name)) for name in [stem, stem + ".stderr"]})
    write_json(directory / (stem + ".command.json"), item)
    return item


def fixture(out):
    roots = dict(candidate="/fixture", baseline="/fixture/drafts/rust-pair-scout-baseline")
    original_out = Path("/fixture/drafts/profile")
    driver = b"fixture scout driver; not production source\n"
    (out / "original-driver.py").write_bytes(driver)
    (out / "original-profile-driver.py").write_bytes(b"fixture profile driver; not production source\n")
    (out / "original-native-preflight.sh").write_text("fixture native preflight, not executable evidence\n")
    (out / "features.txt").write_text("flags: avx2 fma fixture-only\n")
    retained, identities = {}, {}
    for role in ["baseline", "candidate"]:
        directory = out / "probe" / role
        directory.mkdir(parents=True)
        inputs, sources = {}, {}
        for name in profile.scout.PROBE_INPUTS:
            (directory / name).write_text("fixture " + name + "\n")
            source = "drafts/rust-pair-scout-probe/" + name
            digest = profile.scout.digest(directory / name)
            sources[source] = digest
            inputs[source] = dict(path="probe/" + role + "/" + name, sha256=digest)
        (directory / "build.log").write_text("fixture build diagnostic\n")
        retained[role] = dict(inputs=inputs, diagnostics={"build.log": dict(path="probe/" + role + "/build.log", sha256=profile.scout.digest(directory / "build.log"))})
        identities[role] = dict(head=("1" if role == "baseline" else "2") * 40, dirty="", sources=sources,
                               environment=profile.ENV, binary=hashlib.sha256((role + " fixture binary").encode()).hexdigest(),
                               driver_sha256=hashlib.sha256(driver).hexdigest())
    preflight = {}
    for name in profile.PREFLIGHT:
        path = out / ("preflight-" + name + ".log")
        path.write_text(f"test result: ok. {11 if name == 'public-counts' else 1} passed; fixture only\n")
        preflight[name] = dict(path=path.name, sha256=profile.scout.digest(path))
    h = dict(schema="diagnostic.native-cpu-sampling.v2", started=1, performance_valid=False,
             scope="fixture-only metadata, no real samples", identities=identities, retained=retained, preflight=preflight,
             profile_driver_sha256=profile.scout.digest(out / "original-profile-driver.py"),
             native_preflight_sha256=profile.scout.digest(out / "original-native-preflight.sh"), features_sha256=profile.scout.digest(out / "features.txt"),
             host="Linux-fixture", rustc="host: x86_64-unknown-linux-gnu\nrelease: 1.93.1\n",
             available_affinity=[0, 1, 2, 3], affinity=[0], cases=[list(t) for t in profile.CASES],
             roots=roots, original_out=str(original_out), binaries={},
             owner=dict(uid=1000, gid=1000), perf=dict(path="/usr/lib/linux-tools/fixture/perf", sha256="f" * 64, version="fixture"),
             event="cpu-clock:u", frequency=499, call_graph="dwarf,8192", observations=101)
    binaries = out / "binaries"
    binaries.mkdir()
    for role in ["baseline", "candidate"]:
        (binaries / (role + ".bin")).write_bytes((role + " fixture binary").encode())
        path = str(original_out / "binaries" / (role + ".bin"))
        h["binaries"][role] = dict(
            notes=output_command(binaries, role + ".notes.txt", ["readelf", "-n", path], "Build ID: " + role.encode().hex() + "\n"),
            symbols=output_command(binaries, role + ".symbols.txt", ["nm", "-SC", "--size-sort", path], "fixture symbol\n"))
    events = []
    for index, (role, policy) in enumerate(profile.CASES):
        directory = out / (str(index) + "-" + role + "-" + policy)
        directory.mkdir()
        data = original_out / directory.name / "perf.data"
        e = dict(index=index, role=role, policy=policy, directory=directory.name, performance_valid=False)
        validation = dict(shots=1024, policy=policy, status="ok", seeds=4, exact_records_counts_rng=True, native_counts_rng=True)
        result = dict(shots=1024, policy=policy, route="native", observations=[dict(calls=10, elapsed_ns=50_000_000, ns_per_call=5_000_000)] * 101)
        e["validate"] = output_command(directory, "validation.json", profile.probe_args(roots[role], policy, "validate", 7), json.dumps(validation))
        e["record"] = output_command(directory, "instrumented-result.json", profile.record_args(h["perf"]["path"], 0, roots[role], policy, data, "/fixture/benchmarks/near_clifford/native_cpu_profile.py"), json.dumps(result))
        e["ownership"] = output_command(directory, "ownership.log", profile.ownership_args(1000, 1000, data), "")
        write_json(directory / "task.json", dict(pid=123, executable=profile.probe_args(roots[role], policy, "bench", 101)[0], binary_sha256=identities[role]["binary"], affinity=[0]))
        e["task_sha256"] = profile.scout.digest(directory / "task.json")
        # This raw placeholder is intentionally not a real perf file. Tests skip
        # Git/tool replay explicitly; production/default verification requires both.
        (directory / "perf.data").write_bytes(b"PERFILE2fixture-not-real-perf-data")
        e["data_sha256"] = profile.scout.digest(directory / "perf.data")
        texts = {"perf-script.txt": "".join(f"counts-path-abl 123/123 [000] 1.{i:06d}: cpu-clock:u: 123 fake (counts-path-ablation)\n" for i in range(100)),
                 "perf-report.txt": "fixture report\n", "perf-header.txt": "fixture header\n",
                 "perf-buildids.txt": role.encode().hex() + " " + profile.probe_args(roots[role], policy, "bench", 101)[0] + "\n"}
        e["exports"] = {stem: output_command(directory, stem, [h["perf"]["path"], *cmd, "-i", str(data)], texts[stem]) for stem, cmd in profile.EXPORTS.items()}
        events.append(e)
    write_json(out / "header.json", h)
    c = dict(finished=2, identities_after=copy.deepcopy(identities), events=4, performance_valid=False)
    seal(out, h, c, events)
    return h, c, events


def seal(out, h, c, events):
    write_json(out / "header.json", h)
    (out / "events.jsonl").write_text("".join(json.dumps(e) + "\n" for e in events))
    c["header_sha256"] = profile.scout.digest(out / "header.json")
    c["events_sha256"] = profile.scout.digest(out / "events.jsonl")
    write_json(out / "closure.json", c)


class ProfileContract(unittest.TestCase):
    def test_fixture_metadata_only_passes_explicit_non_execution_check(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            fixture(out)
            profile.verify(out, git_sources=False, replay=False)

    def test_resealed_semantic_corruptions_are_rejected(self):
        for kind in ["scope", "validation", "duration", "observations", "samples", "buildid", "cpu_command", "exit", "output_inventory", "output_path", "source_closure", "source_root", "binary", "event_order", "sample_cpu", "sample_pid", "sample_tid", "sample_executable", "sample_comm", "task_pid", "task_binary", "task_affinity", "escaped_out", "timeout", "cpu_attribute"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                out = Path(temporary)
                h, c, events = fixture(out)
                e = events[0]
                directory = out / e["directory"]
                if kind == "scope": h["performance_valid"] = True
                elif kind == "source_closure": c["identities_after"]["candidate"]["binary"] = "0" * 64
                elif kind == "source_root": h["roots"]["baseline"] = "/unexpected"
                elif kind == "escaped_out": h["original_out"] = "/fixture/drafts/../../escaped-profile"
                elif kind.startswith("task_"):
                    body = profile.read(directory / "task.json")
                    if kind == "task_pid": body["pid"] = 999
                    elif kind == "task_binary": body["binary_sha256"] = "0" * 64
                    else: body["affinity"] = [999]
                    write_json(directory / "task.json", body)
                    e["task_sha256"] = profile.scout.digest(directory / "task.json")
                elif kind.startswith("sample_"):
                    stem = "perf-script.txt"
                    body = (directory / stem).read_text()
                    changes = {"sample_cpu": ("[000]", "[999]"), "sample_pid": ("123/123", "999/123"),
                               "sample_tid": ("123/123", "123/999"), "sample_executable": ("(counts-path-ablation)", "(/usr/bin/unrelated)"),
                               "sample_comm": ("counts-path-abl", "unrelated-app")}
                    body = body.replace(*changes[kind])
                    e["exports"][stem] = output_command(directory, stem, e["exports"][stem]["command"], body)
                elif kind == "binary": (out / "binaries/baseline.bin").write_bytes(b"corrupt")
                elif kind == "event_order": events[0], events[1] = events[1], events[0]
                elif kind in ["validation", "duration", "observations"]:
                    stem, key = ("validation.json", "validate") if kind == "validation" else ("instrumented-result.json", "record")
                    body = profile.read(directory / stem)
                    if kind == "validation": body["native_counts_rng"] = False
                    elif kind == "duration": body["observations"][0]["elapsed_ns"] = 1
                    else: body["observations"].pop()
                    e[key] = output_command(directory, stem, e[key]["command"], json.dumps(body))
                elif kind in ["samples", "buildid"]:
                    stem = "perf-script.txt" if kind == "samples" else "perf-buildids.txt"
                    text = "# cpu-clock:u header only, no samples\n" if kind == "samples" else "wrong executable build-id\n"
                    e["exports"][stem] = output_command(directory, stem, e["exports"][stem]["command"], text)
                else:
                    item = e["record"]
                    if kind == "cpu_command": item["command"][item["command"].index("-c") + 1] = "1"
                    elif kind == "cpu_attribute": item["command"].remove("--sample-cpu")
                    elif kind == "exit": item["exit_code"] = 1
                    elif kind == "timeout": item["timed_out"] = True
                    elif kind == "output_inventory": item["outputs"].pop("instrumented-result.json.stderr")
                    else: item["outputs"]["instrumented-result.json"]["path"] = "../escape"
                    write_json(directory / "instrumented-result.json.command.json", item)
                seal(out, h, c, events)
                with self.assertRaises(ValueError):
                    profile.verify(out, git_sources=False, replay=False)

    def test_producer_retains_current_preflight_logs_with_exact_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "drafts").mkdir()
            out = root / "retained"
            out.mkdir()
            originals = {}
            for name in profile.PREFLIGHT:
                body = f"test result: ok. {11 if name == 'public-counts' else 1} passed; fixture only\n".encode()
                (root / ("drafts/x86-scout-" + name + ".log")).write_bytes(body)
                originals[name] = body
            with patch.object(profile, "ROOT", root):
                retained = profile.retain_native_preflight(out)
            self.assertEqual(set(retained), set(profile.PREFLIGHT))
            for name, body in originals.items():
                target = "preflight-" + name + ".log"
                self.assertEqual((out / target).read_bytes(), body)
                self.assertEqual(retained[name], dict(path=target, sha256=hashlib.sha256(body).hexdigest()))

    def test_producer_rejects_old_or_unexpected_counts_before_retention(self):
        for count in (0, 10, 12):
            with self.subTest(count=count), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "drafts").mkdir()
                out = root / "retained"
                out.mkdir()
                for name in profile.PREFLIGHT:
                    passed = count if name == "public-counts" else 1
                    (root / ("drafts/x86-scout-" + name + ".log")).write_text(f"test result: ok. {passed} passed; fixture only\n")
                with patch.object(profile, "ROOT", root):
                    with self.assertRaisesRegex(ValueError, "native preflight missing: public-counts"):
                        profile.retain_native_preflight(out)
                self.assertFalse((out / "preflight-public-counts.log").exists())

    def test_current_profile_rejects_resealed_old_public_counts_totals(self):
        for count in (0, 10, 12):
            with self.subTest(count=count), tempfile.TemporaryDirectory() as temporary:
                out = Path(temporary)
                h, c, events = fixture(out)
                path = out / "preflight-public-counts.log"
                path.write_text(f"test result: ok. {count} passed; fixture only\n")
                h["preflight"]["public-counts"]["sha256"] = profile.scout.digest(path)
                seal(out, h, c, events)
                with self.assertRaisesRegex(ValueError, "native gate result"):
                    profile.verify(out, git_sources=False, replay=False)

    def test_current_profile_does_not_relabel_historical_v1_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            h, c, events = fixture(out)
            h["schema"] = "diagnostic.native-cpu-sampling.v1"
            seal(out, h, c, events)
            with self.assertRaisesRegex(ValueError, "instrumented timings"):
                profile.verify(out, git_sources=False, replay=False)

    def test_timeout_stops_descendant_and_preserves_failed_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            pidfile = out / "descendant.pid"
            script = "import subprocess,time,pathlib,sys; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); pathlib.Path(sys.argv[1]).write_text(str(p.pid)); print('partial output',flush=True); time.sleep(30)"
            try:
                with self.assertRaises(ValueError):
                    profile.command([sys.executable, "-c", script, str(pidfile)], out, "timeout.log", timeout=0.5)
                receipt = profile.read(out / "timeout.log.command.json")
                self.assertTrue(receipt["timed_out"])
                self.assertNotEqual(receipt["exit_code"], 0)
                self.assertEqual((out / "timeout.log").read_text(), "partial output\n")
                child = int(pidfile.read_text())
                # A reparented zombie is already stopped, and its parent init owns
                # reaping. Never mistake kill(pid, 0) for a running descendant.
                for _ in range(50):
                    state = subprocess.run(["ps", "-p", str(child), "-o", "stat="], capture_output=True, text=True, timeout=2).stdout.strip()
                    if not state or state.startswith("Z"):
                        break
                    time.sleep(0.02)
                self.assertTrue(not state or state.startswith("Z"), state)
            finally:
                if pidfile.exists():
                    try:
                        os.kill(int(pidfile.read_text()), signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    @unittest.skipUnless(hasattr(os, "sched_getaffinity"), "native Linux exec contract")
    def test_task_receipt_survives_exec_with_same_pid_and_binary(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            receipt = out / "task.json"
            receipt.touch(mode=0o600)
            result = subprocess.run([sys.executable, str(Path(profile.__file__)), "--exec-probe", str(receipt), "--",
                                     sys.executable, "-c", "import os; print(os.getpid())"], capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
            task = profile.read(receipt)
            self.assertEqual(task["pid"], int(result.stdout))
            self.assertEqual(task["binary_sha256"], profile.scout.digest(Path(sys.executable)))
            self.assertEqual(task["affinity"], sorted(os.sched_getaffinity(0)))


if __name__ == "__main__":
    unittest.main()
