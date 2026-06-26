import unittest

from benchmarks.bb_circuit_bposd_compare.verify_replay import verify_rows


CASE_ID = "bb90-p006-c10-seed12345-order7-hard-syndrome"
PREDICTION = "[false,true,false,true,false,false,false,true]"
SUPPORT = "[5,8,14]"


def fake_fixture() -> dict[str, object]:
    return {
        "case_id": CASE_ID,
        "basis": "Z",
        "syndrome_support": [5, 8, 14],
        "expected_sampled_logical": [False, True, False, True, False, False, False, True],
    }


def make_row(decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": CASE_ID,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": "bb90",
        "p": "0.006",
        "num_cycles": "10",
        "num_trials": "1",
        "seed": "12345",
        "bp_method": "ms",
        "max_iter": "10000",
        "osd_method": "osd_cs",
        "osd_order": "7",
        "basis": "Z",
        "syndrome_weight": "3",
        "syndrome_support": SUPPORT,
        "logical_prediction": PREDICTION,
        "expected_logical": PREDICTION,
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "1" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "10000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "4100" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "4101" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


class VerifyReplayTest(unittest.TestCase):
    def test_verify_rows_accepts_paired_hard_replay(self) -> None:
        self.assertEqual(
            verify_rows(
                [make_row("rbposd"), make_row("ldpc_bposd")],
                fixture=fake_fixture(),
            ),
            [],
        )

    def test_verify_rows_rejects_unpaired_syndrome_metadata(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row("ldpc_bposd", syndrome_support="[5,8,15]"),
            ],
            fixture=fake_fixture(),
        )
        self.assertIn("Rust/Python replay is no longer paired", "\n".join(errors))

    def test_verify_rows_rejects_syndrome_that_matches_pair_but_not_fixture(self) -> None:
        rows = [
            make_row("rbposd", syndrome_support="[5,8,15]"),
            make_row("ldpc_bposd", syndrome_support="[5,8,15]"),
        ]
        errors = verify_rows(rows, fixture=fake_fixture())
        self.assertIn(
            "hard replay row no longer matches checked-in fixture syndrome",
            "\n".join(errors),
        )

    def test_verify_rows_rejects_logical_prediction_mismatch(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row(
                    "ldpc_bposd",
                    logical_prediction="[true,true,false,true,false,false,false,true]",
                ),
            ],
            fixture=fake_fixture(),
        )
        self.assertIn(
            "Rust/Python logical predictions do not match", "\n".join(errors)
        )

    def test_verify_rows_rejects_skipped_python_without_allow_missing(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row(
                    "ldpc_bposd",
                    status="skipped",
                    setup_seconds="",
                    decode_seconds="",
                    run_seconds="",
                    logical_prediction="",
                    error=(
                        "python dependency unavailable for ldpc_bposd replay: "
                        "No module named 'ldpc'"
                    ),
                ),
            ],
            fixture=fake_fixture(),
        )
        self.assertIn("Python ldpc_bposd replay row is skipped", "\n".join(errors))

    def test_verify_rows_allows_skipped_python_with_allow_missing(self) -> None:
        rows = [
            make_row("rbposd"),
            make_row(
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_prediction="",
                error=(
                    "python dependency unavailable for ldpc_bposd replay: "
                    "No module named 'ldpc'"
                ),
            ),
        ]
        self.assertEqual(
            verify_rows(rows, allow_missing_python=True, fixture=fake_fixture()), []
        )

    def test_verify_rows_rejects_missing_rust_counters(self) -> None:
        errors = verify_rows(
            [make_row("rbposd", gf2_solve_count=""), make_row("ldpc_bposd")],
            fixture=fake_fixture(),
        )
        self.assertIn(
            "Rust rbposd replay row is missing OSD/GF(2) counter fields",
            "\n".join(errors),
        )


if __name__ == "__main__":
    unittest.main()
