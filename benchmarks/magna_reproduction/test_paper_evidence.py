"""Admission, corruption and timing checks for the offline paper bundle."""

from copy import deepcopy
import hashlib
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import paper_evidence as evidence
from paper_evidence_report import cultivation_efficiency, decoder_timings, paired_timing
from remaining_contracts import reference


def deposit_plan(index):
    pinned = reference.load_manifest()
    paper = pinned["paper"]
    return dict(schema=evidence.SCHEMA, paper=paper["arxiv"], archive_sha256=paper["archive_sha256"],
                historical_kernel=paper["kernel_commit"], upstream_commit=pinned["upstream"]["commit"],
                reference_sha256=reference.digest(evidence.HERE/"reference.json"),
                ablation_source_sha256="0"*64,
                captures={name: {} for name in evidence.CAMPAIGNS}, index_sha256=hashlib.sha256(index).hexdigest())


class PaperEvidenceTests(unittest.TestCase):
    def setUp(self):
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.raw = self.root/"raw"
        for name in (*evidence.CAMPAIGNS, "provenance"):
            root = self.raw/name
            root.mkdir(parents=True)
            (root/"receipt.json").write_text('{"value": "original"}\n')
        self.failure = self.raw/"cultivation/retained-failure.json"
        self.failure.write_text('{"status": "failed"}\n')
        self.index = evidence.artifact_inventory(self.raw)
        self.index_data = evidence.canonical_json(self.index)
        self.plan = deposit_plan(self.index_data)
        self.sealed = self.root/"sealed"
        (self.sealed/"derived").mkdir(parents=True)
        (self.sealed/"index.json").write_bytes(self.index_data)
        (self.sealed/"plan.json").write_bytes(evidence.canonical_json(self.plan))

    def test_deleted_failed_attempt_is_a_bundle_error(self):
        self.assertEqual(evidence.verify_inventory(self.raw, self.index), 5)
        self.failure.unlink()
        with self.assertRaisesRegex(ValueError, "missing .*retained-failure"):
            evidence.check_bundle(self.plan, self.root)
        (self.sealed/"index.json").write_bytes(evidence.canonical_json(evidence.artifact_inventory(self.raw)))
        with self.assertRaisesRegex(ValueError, "sealed evidence index hash"):
            evidence.check_bundle(self.plan, self.root)

    def test_modified_input_and_mixed_contract_are_detected(self):
        receipt = self.raw/"decoder-workloads/receipt.json"
        receipt.write_text('{"output_contract": "different sample contract"}\n')
        with self.assertRaisesRegex(ValueError, "raw bytes changed: decoder-workloads/receipt.json"):
            evidence.check_bundle(self.plan, self.root)

    def test_symlink_inside_raw_inventory_is_rejected(self):
        receipt = self.raw/"bh/receipt.json"
        receipt.unlink()
        receipt.symlink_to("../cultivation/receipt.json")
        with self.assertRaisesRegex(ValueError, "special file in evidence inventory"):
            evidence.verify_inventory(self.raw, self.index)

    def test_failed_recomputation_leaves_the_raw_bundle_retryable(self):
        self.sealed.rename(self.root/"previous-seal")
        plan = self.plan.copy()
        del plan["index_sha256"]
        archive = self.root/"original.zip"
        with patch("paper_evidence_deposit.extract"), patch.object(evidence, "generate", side_effect=ValueError("bad raw")):
            with self.assertRaisesRegex(ValueError, "bad raw"):
                evidence.seal_bundle(plan, self.root, archive)
        self.assertFalse(self.sealed.exists())
        self.assertEqual(evidence.verify_inventory(self.raw, self.index), 5)
        output = dict(artifacts=5, cells=dict(selected=2, completed=1, failed=1))
        with patch("paper_evidence_deposit.extract"), patch.object(evidence, "generate", return_value=(output, {})):
            self.assertEqual(evidence.seal_bundle(plan, self.root, archive)["status"], "SEALED")
            self.assertEqual(evidence.check_bundle(self.plan, self.root)["status"], "PASS")

    def test_weighted_covariance_export_must_match_recomputation(self):
        summary = dict(cells=dict(selected=2, completed=1, failed=1),
                       decoder_joint_estimates=dict(logical_covariance={"conditional": [[1, 0], [0, 1]]}))
        derived = {"regenerated.json": evidence.canonical_json(summary)}
        (self.sealed/"derived/regenerated.json").write_bytes(derived["regenerated.json"])
        with patch.object(evidence, "generate", return_value=(summary, derived)):
            result = evidence.check_bundle(self.plan, self.root)
            self.assertEqual(result, dict(status="PASS", artifacts=5, selected=2, completed=1, failed=1))
            changed = deepcopy(summary)
            changed["decoder_joint_estimates"]["logical_covariance"]["conditional"][0][1] = 1
            (self.sealed/"derived/regenerated.json").write_bytes(evidence.canonical_json(changed))
            with self.assertRaisesRegex(ValueError, "weighted covariance differs"):
                evidence.check_bundle(self.plan, self.root)

    def test_decoder_times_are_subtracted_per_trial_before_median(self):
        trials = [(10, 9, 1), (20, 3, 1), (30, 25, 1)]
        self.assertEqual(paired_timing(trials), (20, 5))
        history = dict(trials=[dict(wall_seconds=x, decoder_seconds=d, attempts=n) for x, d, n in trials],
                       median_seconds_per_attempt=20, median_decoder_subtracted_seconds_per_attempt=5)
        for row in history["trials"]:
            row["internal_timings"] = dict(preparation_seconds=1)
        cases = [dict(timings=dict(sampling_seconds=x, decoder_seconds=d, preparation_seconds=1), attempts=n)
                 for x, d, n in trials]
        timing = dict(trials=cases, historical=history, median_sampling_seconds_per_attempt=20,
                      median_decoder_subtracted_seconds_per_attempt=5)
        observed = {"code-switching": {"timing": {"dk": timing}}}
        output = decoder_timings(observed)
        self.assertEqual(next(output)["decoder_subtracted_seconds_per_attempt"], 5)
        self.assertEqual(next(output)["decoder_subtracted_seconds_per_attempt"], 5)
        timing["median_decoder_subtracted_seconds_per_attempt"] = 20 - 9
        with self.assertRaisesRegex(ValueError, "subtracted within each trial"):
            next(decoder_timings(observed))

    def test_cultivation_retains_unresolved_precision_and_separate_clock_scopes(self):
        old = dict(p=0.001, acceptance_shots=20, failure_shots=30, failure_positive=6,
                   acceptance_seconds=1, failure_seconds=3, conditional_error=0.1, conditional_error_se=0.05)
        current = dict(p="1/1000", acceptance_draws=20, failure_draws=30, positive_failure_draws=0,
                       failure_ess=8, time_to_observed_precision=dict(
                           acceptance_worker_seconds=2, failure_worker_seconds=5, relative_conditional_error_se=None))
        observed = dict(historical_curve={"color5": {"rows": [old]}}, observed={"precision": {"color5": [current]}})
        row, = cultivation_efficiency(observed)
        self.assertEqual(row["rust"]["failure_worker_seconds"], 5)
        self.assertEqual(row["deposit"]["failure_wall_seconds"], 3)
        self.assertEqual(row["deposit"]["positive_failure_draws"], 6)
        self.assertEqual(row["deposit"]["relative_conditional_error_se"], 0.5)
        self.assertIsNone(row["rust"]["relative_conditional_error_se"])


if __name__ == "__main__":
    unittest.main()
