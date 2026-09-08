from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools import check_crate_consumer as checker


class CrateConsumerCheckerTest(unittest.TestCase):
    def test_replay_verifier_rejects_plausible_but_wrong_predictions_and_stats(self):
        import hashlib
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            predictions, stats = root / "predictions.b8", root / "stats.json"
            correct = bytes([0, 1, 1, 0])
            record = {"decoder": "rbposd", "num_shots": 4, "num_detectors": 1,
                      "num_observables": 1, "prediction_bytes": 4,
                      "predictions_sha256": hashlib.sha256(correct).hexdigest()}
            predictions.write_bytes(correct)
            stats.write_text(json.dumps(record))
            checker.verify_replay_outputs(predictions, stats, "rbposd")
            predictions.write_bytes(bytes([0, 0, 0, 0]))
            with self.assertRaisesRegex(checker.ConsumerCheckError, "wrong observable predictions"):
                checker.verify_replay_outputs(predictions, stats, "rbposd")
            predictions.write_bytes(correct)
            record["num_shots"] = 3
            stats.write_text(json.dumps(record))
            with self.assertRaisesRegex(checker.ConsumerCheckError, "num_shots differs"):
                checker.verify_replay_outputs(predictions, stats, "rbposd")

    def test_workspace_snapshot_does_not_share_the_source_lockfile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            checker,
            "workspace_packages",
            return_value={
                "demo": {"manifest_path": str(Path(temporary) / "source/demo/Cargo.toml")}
            },
        ):
            root = Path(temporary)
            source = root / "source"
            package = source / "demo"
            package.mkdir(parents=True)
            (source / "Cargo.toml").write_text("[workspace]\nmembers=['demo']\n", encoding="utf-8")
            (source / "Cargo.lock").write_text("original lock\n", encoding="utf-8")
            (source / "LICENSE").write_text("license\n", encoding="utf-8")
            (package / "Cargo.toml").write_text("[package]\nname='demo'\n", encoding="utf-8")

            snapshot = checker.snapshot_workspace(source, root / "snapshot")
            (snapshot / "Cargo.lock").write_text("cargo changed this\n", encoding="utf-8")

            self.assertEqual((source / "Cargo.lock").read_text(encoding="utf-8"), "original lock\n")

    def test_missing_referenced_viewer_asset_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rstim = root / "rstim"
            assets = rstim / "assets/shot-viewer"
            assets.mkdir(parents=True)
            (rstim / "Cargo.toml").write_text("[package]\nname='rstim'\n", encoding="utf-8")
            (assets / "asset-manifest.json").write_text(
                '{"files":{"index.html":{"sha256":"00"}}}', encoding="utf-8"
            )

            with self.assertRaisesRegex(checker.ConsumerCheckError, "missing viewer asset index.html"):
                checker.validate_package_contents({"rstim": rstim}, root / "source")

    def test_tampered_consumer_success_message_is_rejected(self) -> None:
        metadata = checker.subprocess.CompletedProcess([], 0, '{"packages":[]}', "")
        completed = checker.subprocess.CompletedProcess([], 0, "looks plausible\n", "")
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            checker, "run", side_effect=[metadata, checker.subprocess.CompletedProcess([], 0, "", ""), completed]
        ):
            root = Path(temporary)
            source = root / "examples/rust-consumer/src"
            source.mkdir(parents=True)
            (source.parent / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
            (source / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
            with self.assertRaisesRegex(checker.ConsumerCheckError, "consumer result differs"):
                checker.exercise_consumer(root, root / "target", {}, root / "work")

    def test_minimal_consumer_rejects_transitive_native_solver(self) -> None:
        tree = checker.subprocess.CompletedProcess([], 0, "consumer v0.0.0\nhighs-sys v1.14.2\n", "")
        with mock.patch.object(checker, "run", return_value=tree):
            with self.assertRaisesRegex(checker.ConsumerCheckError, "minimal consumer unexpectedly builds"):
                checker.validate_minimal_graph(Path("Cargo.toml"), {"highs-sys"})

    def test_unexpected_installed_binary_is_rejected_before_quickstart(self) -> None:
        def fake_run(command: list[str], *, cwd: Path):
            if command[1] == "metadata":
                manifest = Path(command[command.index("--manifest-path") + 1]).resolve()
                output = json.dumps(
                    {"packages": [{"name": "rustqec-cli", "manifest_path": str(manifest)}]}
                )
                return checker.subprocess.CompletedProcess(command, 0, output, "")
            install_root = Path(command[command.index("--root") + 1])
            bin_dir = install_root / "bin"
            bin_dir.mkdir(parents=True)
            (bin_dir / "rustqec").write_text("", encoding="utf-8")
            (bin_dir / "rstim").write_text("", encoding="utf-8")
            return checker.subprocess.CompletedProcess(command, 0, "", "")

        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(
            checker, "run", side_effect=fake_run
        ):
            root = Path(temporary)
            cli = root / "rustqec-cli"
            cli.mkdir()
            (cli / "Cargo.toml").write_text("[package]\nname='rustqec-cli'\n", encoding="utf-8")
            with self.assertRaisesRegex(checker.ConsumerCheckError, "installed binary set differs"):
                checker.exercise_installed_cli(root / "target", {"rustqec-cli": cli}, root / "work")


if __name__ == "__main__":
    unittest.main()
