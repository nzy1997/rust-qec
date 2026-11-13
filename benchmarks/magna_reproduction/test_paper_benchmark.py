import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import paper_benchmark as pilot
import native_options


def scalar(coefficients, power=0):
    return dict(coefficients=list(map(str, coefficients)), power=power)


def prescribed():
    return dict(raw_acceptance=scalar([2, 1, 0, -1], -2), raw_error=scalar([1, 0, 0, 0], -3),
                raw_target_success=scalar([3, 2, 0, -2], -3), history_mass=scalar([1, 0, 0, 0]),
                target_normalization=scalar([1, 0, 0, 0]),
                controller_calls=[dict(name="decode", inputs=[False, True, False], outputs=[True])])


def rust_payload(manifest, noisy=False):
    value = prescribed()
    # Different exact normalization of the same algebraic number.
    value["raw_acceptance"] = scalar([4, 2, 0, -2], -3)
    args = pilot.argparse.Namespace(native_transport="prepared", native_max_quotient_rank=12,
                                    native_max_terms=256)
    value["configuration"] = dict(seed=manifest["seed"], history_sampler="AffineSupport" if noisy else "ExactMarginal",
                                  causal_strategy="FixedTemplate", max_terms=manifest["max_terms"], warm_replays=manifest["warm_replays"])
    for family in native_options.FAMILIES:
        value["configuration"][family + "_preparation"] = "native"
        value["configuration"][family + "_preparation_options"] = native_options.policy(args, "native")
        value["prepared_" + family + "_cache"] = dict(cached_queries=1, fixed_resources=1, catalogue_resources=0,
            native_routes=dict(direct=1, product=0, walsh=0, injected=0, injection_budget_skips=0),
            native_transports=dict(prepared=1, budget_skipped=0))
    value.update(compile_ms=1., query_ms=2., compilation=dict(strategy="SharedCommutingLayer"),
        warm_replays=dict(count=manifest["warm_replays"], query_ms=[.1] * manifest["warm_replays"], cache_unchanged=True,
            scope="same-assignment replay with reset entropy/history; not independent Monte Carlo or formal performance rounds"))
    if noisy:
        value["pilot"] = dict(scope=pilot.PILOT_SCOPE, requested=manifest["shots"], completed=manifest["shots"],
            seed=manifest["seed"], failed=0, not_attempted=0, status="complete", failures=[],
            query_attempts=16, network_compilations=0, history_groups=8, sampling_seconds=.25, decoder_seconds=.05,
            metrics=dict(estimates=dict(acceptance=.75, error=.125), standard_errors=dict(acceptance=.1, error=.05)))
    return value


class PaperPilotTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.pinned = pilot.reference.load_manifest()
        self.manifest = pilot.read_json(pilot.HERE / "paper/pilot.json")
        self.manifest["tasks"] = ["bh2-T", "bh2-S"]
        self.receipt = dict(schema="rmagna.paper-pilot.v1", status="recorded",
            manifest_sha256=pilot.json_hash(self.manifest), output_contract=pilot.OUTPUT_CONTRACT,
            before=dict(source_files=475, tasks=31, decoder_files=0), after=dict(source_files=475, tasks=31, decoder_files=0),
            rust_binary_sha256="a" * 64, native_sha256="b" * 64,
            runner_files={n: pilot.reference.digest(pilot.HERE / n) for n in
                          ("paper_benchmark.py", "paper_reference.py", "worker.py", "native_options.py")}, cells=[])
        self.cells = {}
        for task in self.manifest["tasks"]:
            ref = prescribed() | dict(native_sha256="b" * 64)
            noisy = dict(result=dict(schema_version=2, status="complete", seed=self.manifest["seed"], requested=8,
                completed=8, failed=0, not_attempted=0, failures=[],
                estimates=dict(acceptance=.8, error=.2), standard_errors=dict(acceptance=.1, error=.1)), native_sha256="b" * 64)
            stages = {k: dict(status="complete", returncode=0, wall_seconds=.5, payload=v) for k, v in
                (("reference_prescribed", ref), ("rust_prescribed", rust_payload(self.manifest)),
                 ("reference_noisy", noisy), ("rust_noisy", rust_payload(self.manifest, True)))}
            self.cells[task] = dict(task=task, output_contract=pilot.OUTPUT_CONTRACT,
                inputs=pilot.task_inputs(task, self.pinned), outcome="complete", stages=stages)
        self.persist()

    def persist(self):
        self.receipt["cells"] = []
        for task, cell in self.cells.items():
            path = self.root / (task + ".json")
            pilot.save(path, cell)
            self.receipt["cells"].append(dict(task=task, file=path.name, sha256=pilot.reference.digest(path), outcome=cell["outcome"]))
        pilot.save(self.root / "receipt.json", self.receipt)

    def check(self):
        return pilot.check_capture(self.manifest, self.root)

    def test_exact_scalar_comparison_and_independent_noise_streams(self):
        self.assertEqual(self.check(), dict(status="PASS", selected=2, completed=2, negative=0,
                                           scope="bounded_pilot_not_paper_reproduction"))
        # Noisy aggregate values need not equal after eight independent draws.
        pilot.exact_match(prescribed(), rust_payload(self.manifest))
        bad = rust_payload(self.manifest)
        bad["raw_error"] = scalar([0, 0, 0, 0])
        with self.assertRaisesRegex(ValueError, "raw_error mismatch"):
            pilot.exact_match(prescribed(), bad)

    def test_missing_cell_input_hash_and_mixed_output_contract_are_rejected(self):
        self.receipt["cells"].pop()
        pilot.save(self.root / "receipt.json", self.receipt)
        with self.assertRaisesRegex(ValueError, "selected cell"):
            self.check()
        self.persist()
        cell = self.cells["bh2-T"]
        key = next(iter(cell["inputs"]))
        cell["inputs"][key] = "0" * 64
        self.persist()
        with self.assertRaisesRegex(ValueError, "input/decoder identities"):
            self.check()
        cell["inputs"] = pilot.task_inputs("bh2-T", self.pinned)
        cell["output_contract"] = "sampled_full_measurement_records"
        self.persist()
        with self.assertRaisesRegex(ValueError, "output contract"):
            self.check()

    def test_semantic_corruption_fails_even_with_recomputed_artifact_hashes(self):
        baseline = copy.deepcopy(self.cells)
        changes = [
            lambda c: c["stages"]["rust_prescribed"]["payload"]["raw_target_success"].update(power=0),
            lambda c: c["stages"]["rust_prescribed"]["payload"]["controller_calls"][0].update(inputs=[False]),
            lambda c: c["stages"]["rust_prescribed"]["payload"]["controller_calls"][0].update(outputs=[1]),
            lambda c: c["stages"]["rust_prescribed"]["payload"]["warm_replays"].update(cache_unchanged=False),
            lambda c: c["stages"]["rust_noisy"]["payload"]["pilot"].update(completed=7),
            lambda c: c["stages"]["rust_noisy"]["payload"]["pilot"].update(seed=True),
            lambda c: c["stages"]["rust_noisy"]["payload"]["pilot"].update(decoder_seconds=-1),
            lambda c: c["stages"]["reference_noisy"]["payload"].update(native_sha256="c" * 64),
            lambda c: c["stages"]["reference_noisy"]["payload"]["result"]["estimates"].update(error=-1),
        ]
        for change in changes:
            self.cells = copy.deepcopy(baseline)
            change(self.cells["bh2-T"])
            self.persist()
            with self.assertRaises(ValueError):
                self.check()

    def test_negative_cells_are_retained_and_required_success_cannot_be_dropped(self):
        self.manifest["required_completed"] = ["bh2-T"]
        self.receipt["manifest_sha256"] = pilot.json_hash(self.manifest)
        cell = self.cells["bh2-S"]
        cell["stages"]["rust_noisy"] = dict(status="timeout", timeout_seconds=45, wall_seconds=45, stderr="")
        cell["outcome"] = "negative"
        self.persist()
        result = self.check()
        self.assertEqual((result["completed"], result["negative"]), (1, 1))
        partial = rust_payload(self.manifest, True)
        partial["pilot"].update(status="failed", completed=3, failed=1, not_attempted=4,
                              failures=[dict(phase="Sampling", shot=3, kind="TaskError", message="quota")])
        partial["pilot"]["metrics"]["estimates"] = None
        cell["stages"]["rust_noisy"] = dict(status="complete", returncode=0, wall_seconds=.5, payload=partial)
        self.persist()
        self.assertEqual(self.check()["negative"], 1)
        self.manifest["required_completed"] = ["bh2-T", "bh2-S"]
        self.receipt["manifest_sha256"] = pilot.json_hash(self.manifest)
        self.persist()
        with self.assertRaisesRegex(ValueError, "required pilot success"):
            self.check()

    def test_author_decoder_preflight_and_manifest_bounds(self):
        bad = copy.deepcopy(self.manifest)
        bad["shots"] = 1_000_000
        with self.assertRaisesRegex(ValueError, "shots"):
            pilot.validate_manifest(bad, self.pinned)
        bad["shots"] = True
        with self.assertRaisesRegex(ValueError, "shots"):
            pilot.validate_manifest(bad, self.pinned)
        self.manifest["tasks"].append("tqd3")
        self.receipt["manifest_sha256"] = pilot.json_hash(self.manifest)
        pilot.save(self.root / "receipt.json", self.receipt)
        with self.assertRaisesRegex(ValueError, "author decoder"):
            self.check()

    def test_process_timeout_and_malformed_stdout_keep_failure_receipts(self):
        result = pilot.run_process([sys.executable, "-c", "import time; time.sleep(5)"], .1, self.root)
        self.assertEqual(result["status"], "timeout")
        result = pilot.run_process([sys.executable, "-c", "print('not JSON')"], 3, self.root)
        self.assertEqual((result["status"], result["returncode"]), ("failed", 0))
        self.assertIn("parse_error", result)
        self.assertIn("not JSON", result["stdout"])


if __name__ == "__main__":
    unittest.main()
