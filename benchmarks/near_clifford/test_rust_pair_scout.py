"""Exercise persistent scout provenance with synthetic builds, never timing."""
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import rust_pair_scout as scout


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


if __name__ == "__main__":
    unittest.main()
