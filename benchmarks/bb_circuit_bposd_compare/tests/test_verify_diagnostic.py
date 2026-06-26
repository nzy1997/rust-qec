import unittest

from benchmarks.bb_circuit_bposd_compare.cases import DIAGNOSTIC_CASES, CSV_HEADER
from benchmarks.bb_circuit_bposd_compare.verify_diagnostic import verify_rows


def _case(code_id: str):
    return next(case for case in DIAGNOSTIC_CASES if case.code_id == code_id)


def make_row(case, decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": case.case_id,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": case.code_id,
        "p": str(case.p),
        "num_cycles": str(case.num_cycles),
        "num_trials": str(case.num_trials),
        "seed": str(case.seed),
        "bp_method": case.bp_method,
        "max_iter": str(case.max_iter),
        "osd_method": case.osd_method,
        "osd_order": str(case.osd_order),
        "basis": "",
        "syndrome_weight": "",
        "syndrome_support": "",
        "logical_prediction": "",
        "expected_logical": "",
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "2" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "20000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "16" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "1" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    assert set(row) == set(CSV_HEADER)
    return row


def valid_rows() -> list[dict[str, str]]:
    return [
        make_row(_case("bb90"), "rbposd"),
        make_row(_case("bb90"), "ldpc_bposd"),
        make_row(_case("bb144"), "rbposd"),
        make_row(_case("bb144"), "ldpc_bposd"),
    ]


class VerifyDiagnosticTest(unittest.TestCase):
    def test_verify_rows_accepts_paired_diagnostic_cases(self) -> None:
        self.assertEqual(verify_rows(valid_rows()), [])

    def test_verify_rows_rejects_missing_required_csv_columns(self) -> None:
        rows = valid_rows()
        del rows[0]["status"]

        self.assertIn(
            "row is missing required CSV column(s): status",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_mismatched_case_id_pair(self) -> None:
        rows = valid_rows()
        rows[1]["case_id"] = "wrong-case-id"
        errors = "\n".join(verify_rows(rows))
        self.assertIn("Rust/Python diagnostic rows differ on case_id", errors)
        self.assertIn("expected exactly one Python ldpc_bposd diagnostic row", errors)

    def test_verify_rows_rejects_mismatched_pair_config(self) -> None:
        rows = valid_rows()
        rows[1]["num_cycles"] = "11"
        self.assertIn(
            "Rust/Python diagnostic rows differ on num_cycles",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_missing_bb144(self) -> None:
        rows = [row for row in valid_rows() if row["code_id"] != "bb144"]
        self.assertIn(
            "required diagnostic case is missing: bb144",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_extra_unexpected_case(self) -> None:
        rows = valid_rows()
        rows.append(
            make_row(
                _case("bb90"),
                "rbposd",
                case_id="unexpected-diagnostic-case",
            )
        )

        self.assertIn(
            "unexpected diagnostic row is present: unexpected-diagnostic-case rbposd",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_extra_unknown_decoder_impl(self) -> None:
        rows = valid_rows()
        rows.append(make_row(_case("bb90"), "experimental_decoder"))

        self.assertIn(
            f"unexpected diagnostic row is present: {_case('bb90').case_id} experimental_decoder",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_wrong_bb144_point(self) -> None:
        rows = valid_rows()
        for row in rows:
            if row["code_id"] == "bb144":
                row["p"] = "0.005"

        errors = "\n".join(verify_rows(rows))
        self.assertIn("diagnostic row has mismatched p for bb144", errors)

    def test_verify_rows_rejects_completed_row_missing_required_fields(self) -> None:
        rows = valid_rows()
        rows[0]["decode_seconds"] = ""

        self.assertIn(
            "completed diagnostic row missing required timing/logical/status field",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_missing_rust_counters(self) -> None:
        rows = valid_rows()
        rows[0]["gf2_solve_count"] = ""
        self.assertIn(
            "Rust rbposd diagnostic row is missing OSD/GF(2) counter fields",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_negative_rust_counter_timing(self) -> None:
        rows = valid_rows()
        rows[0]["bp_seconds"] = "-0.1"
        self.assertIn(
            "Rust rbposd diagnostic counter/timing field is negative: bp_seconds",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_noninteger_rust_count_field(self) -> None:
        rows = valid_rows()
        rows[0]["gf2_solve_count"] = "1.5"
        self.assertIn(
            "Rust rbposd diagnostic counter field is not an integer: gf2_solve_count",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_rejects_skipped_python_without_allow_missing(self) -> None:
        rows = valid_rows()
        rows[1].update(
            status="skipped",
            setup_seconds="",
            decode_seconds="",
            run_seconds="",
            logical_error_rate="",
            error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
        )
        self.assertIn(
            "Python ldpc_bposd diagnostic row is skipped",
            "\n".join(verify_rows(rows)),
        )

    def test_verify_rows_allows_skipped_python_with_allow_missing(self) -> None:
        rows = valid_rows()
        for row in rows:
            if row["decoder_impl"] == "ldpc_bposd":
                row.update(
                    status="skipped",
                    setup_seconds="",
                    decode_seconds="",
                    run_seconds="",
                    logical_error_rate="",
                    error=(
                        "python dependency unavailable for ldpc_bposd replay: "
                        "No module named 'ldpc'"
                    ),
                )

        self.assertEqual(verify_rows(rows, allow_missing_python=True), [])

    def test_verify_rows_rejects_skipped_python_allow_missing_without_error(self) -> None:
        rows = valid_rows()
        for row in rows:
            if row["decoder_impl"] == "ldpc_bposd":
                row.update(
                    status="skipped",
                    setup_seconds="",
                    decode_seconds="",
                    run_seconds="",
                    logical_error_rate="",
                    error="",
                )

        self.assertIn(
            "Python ldpc_bposd diagnostic row is skipped without an explicit error",
            "\n".join(verify_rows(rows, allow_missing_python=True)),
        )


if __name__ == "__main__":
    unittest.main()
