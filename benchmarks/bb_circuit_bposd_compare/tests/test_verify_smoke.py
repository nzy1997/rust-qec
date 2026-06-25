import unittest

from benchmarks.bb_circuit_bposd_compare.verify_smoke import verify_rows


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


class VerifySmokeTest(unittest.TestCase):
    def test_verify_rows_accepts_paired_smoke_cases(self) -> None:
        rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb72-p0005-c1-t1-seed12345", "ldpc_bposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        self.assertEqual(verify_rows(rows), [])

    def test_verify_rows_flags_missing_python_rows(self) -> None:
        no_python_rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
        ]

        self.assertIn(
            "upstream ldpc/bposd comparison row is missing",
            "\n".join(verify_rows(no_python_rows)),
        )

    def test_verify_rows_flags_missing_required_smoke_case(self) -> None:
        rows_missing_bb90 = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb72-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        self.assertIn(
            "required smoke case is missing: bb90-p0005-c1-t1-seed12345",
            "\n".join(verify_rows(rows_missing_bb90)),
        )

    def test_verify_rows_flags_unpaired_rows(self) -> None:
        unpaired_rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(unpaired_rows)),
        )

    def test_verify_rows_flags_missing_timing_fields(self) -> None:
        missing_timing_rows = [
            make_row(
                "bb72-p0005-c1-t1-seed12345",
                "rbposd",
                decode_seconds="",
            ),
            make_row("bb72-p0005-c1-t1-seed12345", "ldpc_bposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        self.assertIn(
            "completed row missing required timing/logical/status field",
            "\n".join(verify_rows(missing_timing_rows)),
        )

    def test_verify_rows_rejects_mismatched_upstream_pinned_settings(self) -> None:
        mismatched_upstream_rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row(
                "bb72-p0005-c1-t1-seed12345",
                "ldpc_bposd",
                osd_order="6",
            ),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
            make_row("bb90-p0005-c1-t1-seed12345", "ldpc_bposd"),
        ]

        self.assertIn(
            "completed upstream ldpc/bposd row has mismatched pinned setting",
            "\n".join(verify_rows(mismatched_upstream_rows)),
        )

    def test_verify_rows_rejects_skipped_python_rows(self) -> None:
        skipped_python_rows = [
            make_row("bb72-p0005-c1-t1-seed12345", "rbposd"),
            make_row(
                "bb72-p0005-c1-t1-seed12345",
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_error_rate="",
                error="No module named 'ldpc'",
            ),
            make_row("bb90-p0005-c1-t1-seed12345", "rbposd"),
            make_row(
                "bb90-p0005-c1-t1-seed12345",
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_error_rate="",
                error="No module named 'ldpc'",
            ),
        ]

        self.assertIn(
            "no paired Rust/Python diagnostic case is present",
            "\n".join(verify_rows(skipped_python_rows)),
        )


if __name__ == "__main__":
    unittest.main()
