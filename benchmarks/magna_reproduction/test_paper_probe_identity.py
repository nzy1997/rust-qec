import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import paper_proposal_check as checks
from paper_probe_identity import INPUTS, source_identity
import reference


class ProbeIdentityTests(unittest.TestCase):
    def test_retained_executable_is_rejected_after_backend_model_or_build_input_changes(self):
        # A tiny external executable represents a completed build. Retain its
        # exact bytes as each current source/build input changes independently.
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for name in INPUTS:
                path=root/name
                if name == ".cargo":
                    continue
                if name == "rmagna/src":
                    path.mkdir(parents=True)
                    (path/"runtime.rs").write_text("reported_bits\n")
                else:
                    path.parent.mkdir(parents=True,exist_ok=True)
                    path.write_text("initial\n")
            payload=dict(schema="rmagna.paper-proposal-integration.v1",status="PASS",samples=768,
                true_reported_differ=384,decoder_calls=768,rates=3,causal_strategies=2,
                closing_policies=2,patterns_per_policy=64,build_source_sha256=source_identity(root),
                fixture_sha256=reference.digest(root/"rmagna/tests/data/paper-proposal-runtime.json"),
                source_sha256=reference.digest(root/"rmagna/tests/support/paper_proposal_probe.rs"))
            binary=root/"retained-probe"
            binary.write_text("#!/usr/bin/env python3\nprint("+repr(json.dumps(payload))+")\n")
            binary.chmod(0o755)
            binary_hash=reference.digest(binary)
            with patch.object(checks,"HERE",root/"benchmarks/magna_reproduction"), \
                    patch.object(checks,"check_finite",side_effect=lambda _:dict(status="PASS")):
                self.assertEqual(checks.check_with_runtime({},binary)["decoder_runtime_integration"],"PASS")
                for name in ("rmagna/src/runtime.rs", "Cargo.lock", "Cargo.toml", "rmagna/build.rs",
                             "rmagna/examples/evaluate_task.rs",
                             "benchmarks/magna_reproduction/paper_query_work.py",
                             "benchmarks/magna_reproduction/paper_proposals.py",
                             "benchmarks/magna_reproduction/paper_proposal_runtime_vectors.py",
                             "benchmarks/magna_reproduction/paper_proposal_draws.py"):
                    path=root/name
                    before=path.read_bytes()
                    with self.subTest(input=name):
                        path.write_bytes(before+b"changed\n")
                        with self.assertRaisesRegex(ValueError,"stale Rust proposal probe"):
                            checks.check_with_runtime({},binary)
                        path.write_bytes(before)
                for name in ("rmagna/src/new_backend.rs",".cargo/config.toml"):
                    path=root/name
                    path.parent.mkdir(parents=True,exist_ok=True)
                    path.write_text("new input\n")
                    with self.subTest(input=name), self.assertRaisesRegex(ValueError,"stale Rust proposal probe"):
                        checks.check_with_runtime({},binary)
                    path.unlink()
                self.assertEqual(reference.digest(binary),binary_hash)
                self.assertEqual(checks.check_with_runtime({},binary)["status"],"PASS")

    def test_source_identity_rejects_missing_files_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            with self.assertRaisesRegex(ValueError,"missing"):
                source_identity(root)
            (root/"Cargo.toml").symlink_to(root/"missing")
            with self.assertRaisesRegex(ValueError,"symlinks"):
                source_identity(root)


if __name__ == "__main__":unittest.main()
