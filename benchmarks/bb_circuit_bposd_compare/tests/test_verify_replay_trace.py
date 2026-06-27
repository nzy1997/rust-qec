import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.verify_replay_trace import (
    main,
    verify_trace,
)


CASE_ID = "bb90-p006-c10-seed12345-order7-hard-syndrome"
EXPECTED_LOGICAL = [False, True, False, True, False, False, False, True]
RUST_PREDICTED_LOGICAL = [False, True, False, True, False, False, False, True]
PYTHON_PREDICTED_LOGICAL = [True, True, False, True, False, False, False, True]
SYNDROME_SUPPORT = [5, 8, 14]


def make_trace() -> dict[str, object]:
    return {
        "schema_version": 1,
        "case_id": CASE_ID,
        "basis": "Z",
        "syndrome_support": SYNDROME_SUPPORT,
        "syndrome_weight": len(SYNDROME_SUPPORT),
        "expected_sampled_logical": EXPECTED_LOGICAL,
        "classification": "logical_prediction_mismatch",
        "decoders": [
            {
                "decoder_impl": "rbposd",
                "status": "ok",
                "case_id": CASE_ID,
                "basis": "Z",
                "syndrome_support": SYNDROME_SUPPORT,
                "expected_sampled_logical": EXPECTED_LOGICAL,
                "bp_osd_settings": {
                    "bp_method": "ms",
                    "max_iter": 10000,
                    "osd_method": "osd_cs",
                    "osd_order": 7,
                },
                "correction_support": [0, 2, 3],
                "correction_weight": 3,
                "residual_syndrome_matches": True,
                "residual_syndrome_weight": 0,
                "residual_syndrome_support": [],
                "predicted_logical": RUST_PREDICTED_LOGICAL,
            },
            {
                "decoder_impl": "ldpc_bposd",
                "status": "ok",
                "case_id": CASE_ID,
                "basis": "Z",
                "syndrome_support": SYNDROME_SUPPORT,
                "expected_sampled_logical": EXPECTED_LOGICAL,
                "bp_osd_settings": {
                    "bp_method": "ms",
                    "max_iter": 10000,
                    "osd_method": "osd_cs",
                    "osd_order": 7,
                },
                "correction_support": [5, 8, 14],
                "correction_weight": 3,
                "residual_syndrome_matches": True,
                "residual_syndrome_weight": 0,
                "residual_syndrome_support": [],
                "predicted_logical": PYTHON_PREDICTED_LOGICAL,
            },
        ],
    }


class VerifyReplayTraceTest(unittest.TestCase):
    def test_verify_trace_accepts_logical_prediction_mismatch(self) -> None:
        errors = verify_trace(make_trace())
        self.assertEqual(errors, [])

    def test_verify_trace_rejects_missing_python_correction_support(self) -> None:
        trace = make_trace()
        del trace["decoders"][1]["correction_support"]
        self.assertIn(
            "ldpc_bposd missing correction_support",
            "\n".join(verify_trace(trace)),
        )

    def test_verify_trace_rejects_duplicate_rbposd_decoder_entry(self) -> None:
        trace = make_trace()
        trace["decoders"][1]["decoder_impl"] = "rbposd"
        self.assertIn(
            "trace duplicate decoder entry rbposd",
            "\n".join(verify_trace(trace)),
        )

    def test_verify_trace_rejects_unexpected_decoder_entry(self) -> None:
        trace = make_trace()
        trace["decoders"].append(
            {
                "decoder_impl": "mystery_decoder",
                "status": "ok",
                "case_id": CASE_ID,
                "basis": "Z",
                "syndrome_support": SYNDROME_SUPPORT,
                "expected_sampled_logical": EXPECTED_LOGICAL,
                "bp_osd_settings": {
                    "bp_method": "ms",
                    "max_iter": 10000,
                    "osd_method": "osd_cs",
                    "osd_order": 7,
                },
                "correction_support": [0],
                "correction_weight": 1,
                "residual_syndrome_matches": True,
                "residual_syndrome_weight": 0,
                "residual_syndrome_support": [],
                "predicted_logical": RUST_PREDICTED_LOGICAL,
            }
        )
        self.assertIn(
            "trace decoder entries must contain exactly two dict entries",
            "\n".join(verify_trace(trace)),
        )

    def test_verify_trace_rejects_unpaired_syndrome_metadata(self) -> None:
        trace = make_trace()
        trace["decoders"][1]["syndrome_support"] = [5, 8, 15]
        self.assertIn(
            "decoder entries are not paired on syndrome metadata",
            "\n".join(verify_trace(trace)),
        )

    def test_verify_trace_rejects_contradictory_residual_status(self) -> None:
        trace = make_trace()
        trace["decoders"][1]["residual_syndrome_matches"] = True
        trace["decoders"][1]["residual_syndrome_weight"] = 1
        trace["decoders"][1]["residual_syndrome_support"] = [42]
        self.assertIn(
            "ldpc_bposd residual_syndrome_matches contradicts residual_syndrome_weight",
            "\n".join(verify_trace(trace)),
        )

    def test_main_prints_case_basis_and_classification(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "hard_replay_trace.json"
            path.write_text(json.dumps(make_trace()))
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                self.assertEqual(main([str(path)]), 0)

        output = stdout.getvalue()
        self.assertIn(
            "case_id=bb90-p006-c10-seed12345-order7-hard-syndrome",
            output,
        )
        self.assertIn("basis=Z", output)
        self.assertIn("classification=logical_prediction_mismatch", output)


if __name__ == "__main__":
    unittest.main()
