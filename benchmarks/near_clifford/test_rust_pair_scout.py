"""Exercise persistent scout provenance with synthetic builds, never timing."""
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import rust_pair_scout as scout
import x86_pair_followup as followup


class PersistentProvenanceTests(unittest.TestCase):
    def test_two_preparations_retain_each_campaign_and_reject_corruption(self):
        archive = scout.ARCHIVE
        with tempfile.TemporaryDirectory() as temporary:
            candidate = Path(temporary) / "candidate"

            def populate(root):
                (root / "rstim/src").mkdir(parents=True)
                (root / "rstim/src/lib.rs").write_text("unchanged synthetic Rust source\n")
                for name in ["Cargo.toml", "Cargo.lock", "rstim/Cargo.toml"]:
                    (root / name).write_text("unchanged synthetic manifest\n")
                directory = root / "benchmarks/near_clifford/application_counts/fixtures"
                directory.mkdir(parents=True)
                for name in scout.NAMES:
                    (directory / (name + ".stim")).write_text("unchanged synthetic fixture\n")

            populate(candidate)
            frozen = candidate / "frozen-inputs"
            shutil.copytree(archive, frozen)
            baseline_sha = "1" * 40
            builds = []

            def git(args, root=None):
                root = candidate if root is None else root
                if args[:3] == ["git", "worktree", "add"]:
                    populate(Path(args[-2]))
                    return ""
                if args == ["git", "status", "--porcelain"]:
                    return ""
                if args == ["git", "rev-parse", "HEAD"]:
                    return ("2" * 40 if root == candidate else baseline_sha) + "\n"
                self.fail("Unexpected command; actual benchmark execution is forbidden: " + str(args))

            def build(args, **kwargs):
                self.assertEqual(args[:4], ["cargo", "build", "--release", "--locked"])
                builds.append(str(kwargs["cwd"]))
                kwargs["stdout"].write(f"distinct synthetic build log {len(builds)}\n")
                directory = scout.probe(Path(kwargs["cwd"]))
                binary = directory / "target/release/counts-path-ablation"
                binary.parent.mkdir(parents=True, exist_ok=True)
                binary.write_bytes(b"unchanged synthetic binary")
                (directory / "unrelated.tmp").write_text("excluded diagnostic\n")

            def snapshot(roots, name):
                out = candidate / "drafts" / name
                out.mkdir()
                retained = scout.retain_probe_inputs(roots, out)
                identities = {role: scout.identity(root) for role, root in roots.items()}
                scout.verify_retained_inputs(out, identities, retained)
                # Persist the same inventories that a real header records.
                (out / "header.json").write_text(json.dumps(dict(identities=identities, retained=retained)))
                return out, identities, retained

            with patch.object(scout, "ROOT", candidate), patch.object(scout, "ARCHIVE", frozen), patch.object(scout, "run", git), patch.object(scout.subprocess, "run", build):
                roots = scout.prepare(baseline_sha)
                first_out, first_ids, first_retained = snapshot(roots, "warm")
                original = {str(p.relative_to(first_out)): p.read_bytes() for p in first_out.rglob("*") if p.is_file()}
                roots = scout.prepare(baseline_sha)
                second_out, second_ids, second_retained = snapshot(roots, "confirmation")
                self.assertEqual(len(builds), 4)
                self.assertEqual(first_ids, second_ids)
                self.assertEqual(original, {str(p.relative_to(first_out)): p.read_bytes() for p in first_out.rglob("*") if p.is_file()})
                for role in roots:
                    self.assertEqual(len(first_retained[role]["inputs"]), 7)
                    self.assertNotIn("drafts/rust-pair-scout-probe/build.log", first_ids[role]["sources"])
                    self.assertNotIn("drafts/rust-pair-scout-probe/unrelated.tmp", first_ids[role]["sources"])
                    self.assertNotEqual(first_retained[role]["diagnostics"], second_retained[role]["diagnostics"])
                scout.verify_retained_inputs(first_out, first_ids, first_retained)
                scout.verify_retained_inputs(second_out, second_ids, second_retained)
                def synthetic_execute(args, cwd=None):
                    if args[0] == "git":
                        return git(args, cwd)
                    if args == ["rustc", "-Vv"]:
                        return "synthetic toolchain"
                    _, _, shots, policy, route, action, observations = args[1:]
                    result = dict(shots=int(shots), policy=policy)
                    if action == "validate":
                        result.update(status="ok", exact_records_counts_rng=True, native_counts_rng=True, seeds=4)
                    else:
                        result.update(route=route, observations=[dict(calls=1, elapsed_ns=50_000_000, ns_per_call=50_000_000.)] * 7)
                    return json.dumps(result)

                campaign = candidate / "drafts/synthetic-complete-campaign"
                with patch.object(scout, "run", synthetic_execute), patch.object(scout.platform, "platform", return_value="synthetic host"), patch("builtins.print"):
                    scout.collect(roots, campaign)
                scout.verify_closed_campaign(campaign)

                def extract(args, **kwargs):
                    self.assertIn(args[0], ["objdump", "nm"])
                    self.assertEqual(Path(args[-1]).read_bytes(), b"unchanged synthetic binary")
                    kwargs["stdout"].write(b"synthetic extraction, no real assembly or timing\n")

                binaries = candidate / "drafts/retained-binaries"
                with patch.object(followup.subprocess, "run", extract):
                    followup.retain_binaries(campaign, binaries)
                receipt = json.loads((binaries / "receipt.json").read_text())
                self.assertEqual(receipt["identities"], second_ids)
                for role in roots:
                    self.assertEqual((binaries / (role + ".bin")).read_bytes(), b"unchanged synthetic binary")
                    self.assertEqual(receipt["retained"][role]["sha256"], second_ids[role]["binary"])
                binary = scout.probe(candidate) / "target/release/counts-path-ablation"
                binary.write_bytes(b"changed binary after closed measurement")
                with self.assertRaises(ValueError):
                    followup.retain_binaries(campaign, candidate / "drafts/rejected-extraction")
                self.assertFalse((candidate / "drafts/rejected-extraction").exists())
                binary.write_bytes(b"unchanged synthetic binary")
                original_events = (campaign / "events.jsonl").read_bytes()
                original_closure = (campaign / "closure.json").read_bytes()
                for label in ["malformed", "failed-validation", "reordered", "short-observation"]:
                    events = [json.loads(line) for line in original_events.splitlines()]
                    if label == "malformed":
                        events = [{}] * 288
                    if label == "failed-validation":
                        events[0]["result"]["native_counts_rng"] = False
                    if label == "reordered":
                        events[48], events[49] = events[49], events[48]
                    if label == "short-observation":
                        events[48]["result"]["observations"][0]["elapsed_ns"] = 1
                    (campaign / "events.jsonl").write_text(''.join(json.dumps(e) + '\n' for e in events))
                    closure = json.loads(original_closure)
                    closure["events_sha256"] = scout.digest(campaign / "events.jsonl")
                    (campaign / "closure.json").write_text(json.dumps(closure))
                    with patch.object(followup.subprocess, "run", side_effect=AssertionError("extraction must not execute")):
                        with self.assertRaises((ValueError, KeyError)):
                            followup.retain_binaries(campaign, candidate / ("drafts/rejected-" + label))
                    self.assertFalse((candidate / ("drafts/rejected-" + label)).exists())
                (campaign / "events.jsonl").write_bytes(original_events)
                (campaign / "closure.json").write_bytes(original_closure)
                scout.verify_closed_campaign(campaign)
                for out, identities, retained in [(first_out, first_ids, first_retained), (second_out, second_ids, second_retained)]:
                    for relative in ["probe/baseline/main.rs", "probe/candidate/build.log", "original-driver.py"]:
                        path = out / relative
                        original_bytes = path.read_bytes()
                        path.write_bytes(original_bytes + b"corruption")
                        with self.assertRaises(ValueError):
                            scout.verify_retained_inputs(out, identities, retained)
                        path.unlink()
                        with self.assertRaises((ValueError, OSError)):
                            scout.verify_retained_inputs(out, identities, retained)
                        path.write_bytes(original_bytes)
                        scout.verify_retained_inputs(out, identities, retained)

    def test_same_binary_control_closes_full_schedule_and_rejects_distinct_roots(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            info = dict(dirty="", binary="1" * 64)
            executions = []

            def execute(args, cwd=None):
                if args == ["rustc", "-Vv"]:
                    return "synthetic toolchain"
                name, masks, shots, policy, route, action, observations = args[1:]
                executions.append((name, masks, shots, policy, route, action, observations))
                if action == "validate":
                    result = dict(shots=int(shots), policy=policy, status="ok", exact_records_counts_rng=True, native_counts_rng=True, seeds=4)
                else:
                    result = dict(shots=int(shots), policy=policy, route=route, observations=[dict(calls=1, elapsed_ns=50_000_000, ns_per_call=50_000_000.)] * 7)
                return json.dumps(result)

            # Mock all execution/identities: these receipts are synthetic unit
            # fixtures and are never reported as hardware benchmark evidence.
            with patch.object(scout, "identity", return_value=info), patch.object(scout, "retain_probe_inputs", return_value={}), patch.object(scout, "verify_retained_inputs"), patch.object(scout, "run", execute), patch.object(scout.platform, "platform", return_value="synthetic host"), patch("builtins.print"):
                out = root / "null"
                scout.collect(dict(baseline=root, candidate=root), out, same_binary_control=True)
                h = json.loads((out / "header.json").read_text())
                c = json.loads((out / "closure.json").read_text())
                events = [json.loads(line) for line in (out / "events.jsonl").read_text().splitlines()]
                self.assertEqual(h["schema"], "exploratory.same-binary-warm-control.v1")
                self.assertEqual(h["identities"]["baseline"], h["identities"]["candidate"])
                self.assertEqual(c["identities_after"], h["identities"])
                self.assertEqual(c["events"], 288)
                self.assertEqual(c["events_sha256"], scout.digest(out / "events.jsonl"))
                self.assertEqual(len(executions), 288)
                self.assertEqual(sum(e["action"] == "validate" for e in events), 48)
                self.assertEqual(sum(e["action"] == "bench" for e in events), 240)
                for i in range(0, len(executions), 2):
                    self.assertEqual(executions[i], executions[i+1])
                summary = json.loads((out / "summary.json").read_text())
                self.assertEqual(len(summary), 24)
                self.assertTrue(all(s["speedup"] == 1. and s["paired_range"] == [1., 1.] for s in summary))
                with self.assertRaises(ValueError):
                    scout.collect(dict(baseline=root, candidate=root / "other"), root / "rejected", same_binary_control=True)
                self.assertEqual(len(executions), 288)


if __name__ == "__main__":
    unittest.main()
