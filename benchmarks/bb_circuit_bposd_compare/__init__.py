from benchmarks.bb_circuit_bposd_compare.cases import (
    BATCHED_CSV_HEADER,
    BB72_BB144_FULL_CASES,
    BB72_BB144_PLOT_SMOKE_CASES,
    CATALOG_HEADER,
    CSV_HEADER,
    HARD_REPLAY_CASES,
    SMALL_LDPC_CASES,
    SMOKE_CASES,
)

__all__ = [
    "BATCHED_CSV_HEADER",
    "BB72_BB144_FULL_CASES",
    "BB72_BB144_PLOT_SMOKE_CASES",
    "CATALOG_HEADER",
    "CSV_HEADER",
    "HARD_REPLAY_CASES",
    "SMALL_LDPC_CASES",
    "SMOKE_CASES",
    "verify_rows",
    "write_summary",
]


def __getattr__(name: str):
    if name == "verify_rows":
        from benchmarks.bb_circuit_bposd_compare.verify_smoke import verify_rows

        return verify_rows
    if name == "write_summary":
        from benchmarks.bb_circuit_bposd_compare.summary import write_summary

        return write_summary
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
