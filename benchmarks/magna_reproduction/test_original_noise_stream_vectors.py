"""Offline controls for the original finite stream witness generator."""

from contextlib import contextmanager, redirect_stdout
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest import mock

import original_noise_stream_vectors as generator


def fixture():
    path = Path(__file__).resolve().parents[2] / "rmagna/tests/fixtures/original_noise_stream.json"
    return json.loads(path.read_text())


def fake_site(after, qubits, channels, probability, weights=None):
    return SimpleNamespace(after=after, qubits=tuple(qubits), channels=tuple(channels),
                           probability=Fraction(probability),
                           weights=None if weights is None else tuple(map(Fraction, weights)))


class ReferenceModel:
    def __init__(self, sites):
        self.sites = sites

    def draw(self, random, **options):
        return generator.result_tuple(generator.reference_draw(self.sites, random, **options))


class WrongModel(ReferenceModel):
    def draw(self, random, **options):
        faults, flips, weight = super().draw(random, **options)
        return faults, flips, weight + 1


class OriginalNoiseStreamTests(unittest.TestCase):
    def test_complete_original_method_has_a_valid_independent_control(self):
        expected = fixture()
        cases, proof = generator.run_original(expected["source"]["test_source"], fake_site, ReferenceModel)
        self.assertEqual(cases, expected["cases"])
        self.assertEqual(proof["original_assertions"], 960)
        self.assertEqual(proof["draws"], 480)
        self.assertEqual(proof["range_calls"], 3076)
        self.assertTrue(proof["all_per_draw_python_states_equal"])

    def test_wrong_native_oracle_with_the_correct_range_protocol_is_rejected(self):
        # WrongModel consumes the complete correct range protocol, then corrupts
        # only the returned exact likelihood. Rejection must inspect the value.
        with self.assertRaisesRegex(RuntimeError, "wrong native oracle"):
            generator.run_original(fixture()["source"]["test_source"], fake_site, WrongModel)

    def test_original_source_digest_and_portable_host_identity_separation(self):
        document = fixture()
        source = document["source"]
        self.assertEqual(hashlib.sha256(source["test_source"].encode()).hexdigest(),
                         source["test_source_sha256"])
        self.assertNotIn("native", document)
        self.assertNotIn("producer", document)
        self.assertNotIn("upstream_root", source)
        self.assertEqual(source["test_id"], generator.TEST_ID)

    def test_existing_outputs_links_and_aliases_refuse_before_capture(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            upstream = root / "fake-upstream"
            upstream.mkdir()
            existing = root / "existing"
            existing.write_bytes(b"preserve existing bytes")
            broken = root / "broken"
            broken.symlink_to(root / "absent-target")
            parent = root / "alias"
            parent.symlink_to(root, target_is_directory=True)
            fresh = root / "fresh"
            for output, receipt in [(existing, fresh), (fresh, existing), (broken, fresh),
                                    (fresh, broken), (fresh, parent / "fresh"),
                                    (upstream / "new-output", fresh)]:
                with self.subTest(output=output, receipt=receipt), \
                     mock.patch.object(sys, "argv", ["generator", "--upstream", str(upstream),
                                                    "--out", str(output), "--receipt", str(receipt)]), \
                     mock.patch.object(generator, "capture") as capture:
                    with self.assertRaises(ValueError):
                        generator.main()
                    capture.assert_not_called()
            self.assertEqual(existing.read_bytes(), b"preserve existing bytes")
            self.assertTrue(broken.is_symlink())
            self.assertFalse((root / "absent-target").exists())
            self.assertFalse(fresh.exists())
            self.assertEqual(list(upstream.iterdir()), [])

    def test_valid_publication_keeps_host_receipt_separate_and_hashes_exact_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output, receipt = root / "witness.json", root / "host.json"
            document = dict(schema="portable", cases=[])
            with mock.patch.object(sys, "argv", ["generator", "--upstream", str(root / "input"),
                                               "--out", str(output), "--receipt", str(receipt)]), \
                 mock.patch.object(generator, "capture", return_value=(document, {"native": "host-only"})), \
                 redirect_stdout(io.StringIO()):
                generator.main()
            self.assertEqual(json.loads(output.read_text()), document)
            host = json.loads(receipt.read_text())
            self.assertEqual(host["native"], "host-only")
            self.assertEqual(host["fixture"], dict(sha256=hashlib.sha256(output.read_bytes()).hexdigest(),
                                                 bytes=output.stat().st_size))

    def failure_control(self, corrupt):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            test = root / generator.TEST_FILE
            test.parent.mkdir(parents=True)
            data = fixture()["source"]["test_source"].encode()
            test.write_bytes(data)
            binary = root / "fake-native-DATA"
            binary.write_bytes(b"never loaded or executed")
            expected = hashlib.sha256(data).hexdigest()
            native_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
            manifest = dict(upstream=dict(commit="mock", files={generator.TEST_FILE: expected}))
            observations = []

            def verify(source, supplied):
                observations.append(source)
                self.assertEqual(supplied, manifest)
                if hashlib.sha256(test.read_bytes()).hexdigest() != expected:
                    raise generator.reference.ReferenceError("source changed on failed capture")
                return {"verified": 1}

            @contextmanager
            def environment(source):
                yield SimpleNamespace(native=SimpleNamespace(__file__=str(binary)),
                                      manifest=manifest, manifest_sha256="mock-manifest",
                                      native_sha256=native_hash)

            def failed(*args):
                if corrupt == "source":
                    test.write_bytes(data + b"\n# isolated corruption control\n")
                if corrupt == "binary":
                    binary.write_bytes(b"isolated changed fake native DATA")
                raise RuntimeError("synthetic capture failure")

            magna, noise = ModuleType("magna"), ModuleType("magna.noise")
            magna.__path__ = []
            noise.NoiseSite, noise.NoiseModel = fake_site, ReferenceModel
            with mock.patch.dict(sys.modules, {"magna": magna, "magna.noise": noise}), \
                 mock.patch.object(generator.reference, "load_manifest", return_value=manifest), \
                 mock.patch.object(generator.reference, "verify_source", side_effect=verify), \
                 mock.patch.object(generator, "native_reference", environment), \
                 mock.patch.object(generator, "run_original", side_effect=failed):
                if corrupt is None:
                    with self.assertRaisesRegex(RuntimeError, "synthetic capture failure"):
                        generator.capture(root)
                else:
                    with self.assertRaises(generator.reference.ReferenceError):
                        generator.capture(root)
            self.assertEqual(len(observations), 2)

    def test_source_hashes_are_checked_even_when_capture_fails(self):
        self.failure_control(None)

    def test_altered_source_on_failed_capture_is_rejected(self):
        self.failure_control("source")

    def test_altered_native_on_failed_capture_is_rejected(self):
        self.failure_control("binary")


if __name__ == "__main__":
    unittest.main()
