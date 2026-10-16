import csv
from dataclasses import replace
from unittest import mock

from benchmarks.bb_circuit_bposd_compare.cases import (
    BB72_BB144_FULL_CASES,
    BB72_BB144_PLOT_SMOKE_CASES,
)
from benchmarks.bb_circuit_bposd_compare.run_compare import (
    main,
    run_batched_suite,
)


def _fake_export(case):
    trials = []
    for trial_index in range(case.num_trials):
        failed = trial_index == 0
        trials.append(
            {
                "z_syndrome": [failed],
                "x_syndrome": [False],
                "z_logical": [failed],
                "x_logical": [False],
            }
        )
    return {
        "code_id": case.code_id,
        "physical_error_rate": case.p,
        "num_cycles": case.num_cycles,
        "num_trials": case.num_trials,
        "seed": case.seed,
        "max_bp_iterations": case.max_iter,
        "osd_order": case.osd_order,
        "rust_result": {
            "num_failed_trials": 1,
            "profile": {
                "setup_seconds": 0.10,
                "sample_seconds": 0.02,
                "decode_seconds": 0.30,
                "bp_seconds": 0.20,
                "osd_seconds": 0.10,
                "decode_call_count": case.num_trials * 2,
                "bp_iteration_count": case.num_trials,
                "osd_use_count": 1,
                "osd_candidate_count": 16,
                "gf2_solve_count": 1,
                "gf2_full_elimination_count": 1,
            },
        },
        "z_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[0]],
            "augmented_columns": [[1]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "x_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[0]],
            "augmented_columns": [[]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "trials": trials,
    }


def _fake_python_batch_stats(case, export):
    return {
        "setup_seconds": 0.01,
        "decode_seconds": 0.40,
        "num_failed_trials": 1,
    }


def test_run_batched_suite_writes_aggregate_rows_without_trial_artifacts(tmp_path):
    case = replace(BB72_BB144_PLOT_SMOKE_CASES[0], num_trials=3)

    status = run_batched_suite(
        output_dir=tmp_path,
        cases=(case,),
        batch_size=2,
        rust_exporter=_fake_export,
        python_batch_stats=_fake_python_batch_stats,
    )

    assert status == 0
    assert sorted(path.name for path in tmp_path.iterdir()) == [
        "results.csv",
        "summary.md",
    ]
    rows = list(csv.DictReader((tmp_path / "results.csv").open()))
    assert [row["decoder_impl"] for row in rows] == ["rbposd", "ldpc_bposd"]
    assert {row["case_id"] for row in rows} == {case.case_id}
    assert {row["shots_used"] for row in rows} == {"3"}
    assert {row["logical_errors"] for row in rows} == {"2"}
    assert {row["batches_completed"] for row in rows} == {"2"}
    assert {row["stop_reason"] for row in rows} == {"completed"}
    assert "syndrome" not in (tmp_path / "results.csv").read_text()


def test_run_batched_suite_stops_cleanly_on_wall_budget(tmp_path):
    case = replace(BB72_BB144_PLOT_SMOKE_CASES[0], num_trials=4)
    now = iter([0.0, 0.0, 10.0])

    status = run_batched_suite(
        output_dir=tmp_path,
        cases=(case,),
        batch_size=2,
        wall_budget_seconds=5.0,
        rust_exporter=_fake_export,
        python_batch_stats=_fake_python_batch_stats,
        monotonic=lambda: next(now),
    )

    assert status == 0
    rows = list(csv.DictReader((tmp_path / "results.csv").open()))
    assert {row["shots_used"] for row in rows} == {"2"}
    assert {row["stop_reason"] for row in rows} == {"wall_budget_exhausted"}


def test_run_batched_suite_stops_cleanly_on_error_budget(tmp_path):
    case = replace(BB72_BB144_PLOT_SMOKE_CASES[0], num_trials=10, max_errors=2)

    status = run_batched_suite(
        output_dir=tmp_path,
        cases=(case,),
        batch_size=1,
        rust_exporter=_fake_export,
        python_batch_stats=_fake_python_batch_stats,
    )

    assert status == 0
    rows = list(csv.DictReader((tmp_path / "results.csv").open()))
    assert {row["errors_budget"] for row in rows} == {"2"}
    assert {row["shots_used"] for row in rows} == {"2"}
    assert {row["logical_errors"] for row in rows} == {"2"}
    assert {row["status"] for row in rows} == {"ok"}
    assert {row["stop_reason"] for row in rows} == {"errors_budget_reached"}


def test_run_batched_suite_reports_progress(tmp_path):
    case = replace(BB72_BB144_PLOT_SMOKE_CASES[0], num_trials=2, max_errors=None)
    messages: list[str] = []

    status = run_batched_suite(
        output_dir=tmp_path,
        cases=(case,),
        batch_size=1,
        rust_exporter=_fake_export,
        python_batch_stats=_fake_python_batch_stats,
        progress=messages.append,
    )

    assert status == 0
    assert any("case 1/1 start" in message for message in messages)
    assert any("batch 1" in message and "shots=1/2" in message for message in messages)
    assert any("batch 2" in message and "shots=2/2" in message for message in messages)
    assert any(
        "case 1/1 done" in message
        and "rust_errors=2" in message
        and "ldpc_errors=2" in message
        for message in messages
    )


def test_run_batched_suite_counts_rust_batch_when_python_dependency_is_missing(
    tmp_path,
):
    case = replace(BB72_BB144_PLOT_SMOKE_CASES[0], num_trials=2)

    def _missing_python(_case, _export):
        raise ModuleNotFoundError("No module named 'ldpc'")

    status = run_batched_suite(
        output_dir=tmp_path,
        cases=(case,),
        batch_size=2,
        rust_exporter=_fake_export,
        python_batch_stats=_missing_python,
    )

    assert status == 1
    rows = list(csv.DictReader((tmp_path / "results.csv").open()))
    rust = next(row for row in rows if row["decoder_impl"] == "rbposd")
    python = next(row for row in rows if row["decoder_impl"] == "ldpc_bposd")
    assert rust["shots_used"] == "2"
    assert rust["batches_completed"] == "1"
    assert python["status"] == "skipped"
    assert python["stop_reason"] == "python_dependency_missing"


def test_main_batched_smoke_renders_plot_after_success(tmp_path):
    rust_binary = tmp_path / "rsinter"
    with mock.patch(
        "benchmarks.bb_circuit_bposd_compare.run_compare.run_batched_suite",
        return_value=0,
    ) as run_batched:
        with mock.patch(
            "benchmarks.bb_circuit_bposd_compare.run_compare.subprocess.run",
            return_value=mock.Mock(returncode=0, stdout="", stderr=""),
        ) as run_plot:
            status = main(
                [
                    "--tier",
                    "bb72-bb144-plot-smoke",
                    "--output-dir",
                    str(tmp_path),
                    "--rust-binary",
                    str(rust_binary),
                    "--batch-size",
                    "1",
                ]
            )

    assert status == 0
    run_batched.assert_called_once()
    assert run_batched.call_args.kwargs["cases"] == BB72_BB144_PLOT_SMOKE_CASES
    assert run_batched.call_args.kwargs["rust_binary"] == rust_binary
    run_plot.assert_called_once()
    command = run_plot.call_args.args[0]
    assert command[:2] == [str(rust_binary), "bench"]
    assert "plot-bb-compare-csv" in command
    assert str(tmp_path / "results.csv") in command
    assert str(tmp_path / "bb_circuit_bposd_compare.png") in command


def test_main_full_tier_uses_full_cases_without_default_wall_budget(tmp_path):
    with mock.patch(
        "benchmarks.bb_circuit_bposd_compare.run_compare.run_batched_suite",
        return_value=0,
    ) as run_batched:
        with mock.patch(
            "benchmarks.bb_circuit_bposd_compare.run_compare._render_batched_plot"
        ) as render_plot:
            status = main(["--tier", "full", "--output-dir", str(tmp_path)])

    assert status == 0
    run_batched.assert_called_once()
    assert run_batched.call_args.kwargs["cases"] == BB72_BB144_FULL_CASES
    assert run_batched.call_args.kwargs["wall_budget_seconds"] is None
    render_plot.assert_called_once_with(tmp_path, None)
