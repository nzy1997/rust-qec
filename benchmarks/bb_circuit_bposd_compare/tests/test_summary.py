import tempfile
import unittest
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.summary import write_summary


def make_row(case_id: str, decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case_id.split("-")[0],
        "p": "0.0005",
        "num_cycles": "1",
        "num_trials": "1",
        "seed": "12345",
        "bp_method": "ms",
        "max_iter": "10000",
        "osd_method": "osd_cs",
        "osd_order": "7",
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


class SummaryTest(unittest.TestCase):
    def test_write_summary_includes_decoder_and_timing_columns(self) -> None:
        rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb72-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            out_path = Path(tmpdir) / "summary.md"
            write_summary(rows, out_path)

            summary_text = out_path.read_text()

        self.assertIn("rbposd", summary_text)
        self.assertIn("ldpc_bposd", summary_text)
        self.assertIn("decode_seconds", summary_text)


if __name__ == "__main__":
    unittest.main()
